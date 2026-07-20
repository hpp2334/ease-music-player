//! [`Player`]: the public orchestrator and its worker thread.
//!
//! Each `Player` owns one dedicated worker thread that runs the
//! decode→sink loop. The public API methods are synchronous and
//! non-blocking: they post [`Command`]s onto an mpsc channel and return.
//! The worker drains commands, drives the state machine via
//! [`crate::state::transition`], and emits [`PlayerEvent`]s to the
//! configured sinks.
//!
//! The worker (not the public API) is the sole owner of the decoder and
//! the sink. This keeps the cpal `Stream` on a single thread for its whole
//! lifetime — the discipline cpal/AAudio/CoreAudio require for real-time
//! audio.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    context::{PlayerContext, PlayerHandle},
    decoder::Decoder,
    events::{EventSink, PlayerEvent},
    state::{transition, PlayerState, WorkerEvent},
    AudioSource, CantodeError, Metadata,
};

/// How often the worker emits [`PlayerEvent::PositionChanged`] while
/// playing. 10 Hz matches typical UI polling cadences and keeps the
/// event channel from saturating.
const POSITION_EMIT_INTERVAL: Duration = Duration::from_millis(100);

/// Capacity of the command channel. Small because commands are rare
/// relative to decode work; backpressure is fine (the worker drains fast).
const COMMAND_CHANNEL_CAP: usize = 32;

/// Internal command set posted by the public API to the worker.
pub(crate) enum Command {
    /// Load a fresh source. The worker rebuilds the decoder and primes the
    /// sink. The `SyncSender` lets the worker report the resulting
    /// [`Metadata`] (or error) back to the caller of `load`.
    Load {
        source: Box<dyn AudioSource>,
        reply: mpsc::Sender<LoadResult>,
    },
    Play,
    Pause,
    Stop,
    Seek {
        target: Duration,
        reply: mpsc::Sender<SeekResult>,
    },
    Unload {
        reply: mpsc::Sender<crate::Result<()>>,
    },
    SetVolume(f32),
    Shutdown,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Load { .. } => f.debug_struct("Command::Load").finish_non_exhaustive(),
            Command::Play => write!(f, "Command::Play"),
            Command::Pause => write!(f, "Command::Pause"),
            Command::Stop => write!(f, "Command::Stop"),
            Command::Seek { target, .. } => f
                .debug_struct("Command::Seek")
                .field("target", target)
                .finish_non_exhaustive(),
            Command::Unload { .. } => f.debug_struct("Command::Unload").finish_non_exhaustive(),
            Command::SetVolume(v) => f.debug_tuple("Command::SetVolume").field(v).finish(),
            Command::Shutdown => write!(f, "Command::Shutdown"),
        }
    }
}

/// Result of a `Load` command.
pub(crate) enum LoadResult {
    Ok(Metadata),
    Err(CantodeError),
}

/// Result of a `Seek` command.
pub(crate) enum SeekResult {
    Ok(Duration),
    Err(CantodeError),
}

/// Configuration for an individual [`Player`].
#[derive(Default)]
pub struct PlayerConfig {
    /// Optional per-player event sink, in addition to the
    /// [`PlayerContext`]'s global one.
    pub event_sink: Option<Arc<dyn EventSink>>,
}

/// A handle to one audio playback pipeline.
///
/// Created via [`Player::new`]; owns one worker thread for its whole
/// lifetime. Dropping a `Player` posts a shutdown command and joins the
/// worker. All public methods are non-blocking.
pub struct Player {
    handle: Arc<PlayerHandle>,
    join: Mutex<Option<JoinHandle<()>>>,
    state: Arc<AtomicState>,
    position: Arc<AtomicPosition>,
    duration: Arc<Mutex<Option<Duration>>>,
}

impl Player {
    /// Create a new player attached to `cx`.
    ///
    /// Spawns a dedicated worker thread named `cantode-player-N` and
    /// registers the player in `cx`'s live-player registry.
    pub fn new(cx: &mut PlayerContext) -> Result<Self, CantodeError> {
        Self::with_config(cx, PlayerConfig::default())
    }

    /// Like [`Player::new`] but with per-player overrides.
    pub fn with_config(cx: &mut PlayerContext, config: PlayerConfig) -> Result<Self, CantodeError> {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel(COMMAND_CHANNEL_CAP);
        let state = Arc::new(AtomicState::new(PlayerState::Idle));
        let position = Arc::new(AtomicPosition::new());
        let duration: Arc<Mutex<Option<Duration>>> = Arc::new(Mutex::new(None));

        let decoder_factory = Arc::clone(cx.decoder_factory());
        let cx_event_sink = cx.event_sink().cloned();
        let player_event_sink = config.event_sink.clone();

        let worker_state = state.clone();
        let worker_position = position.clone();
        let worker_duration = duration.clone();

        let name = cx.next_worker_name();
        let join = thread::Builder::new()
            .name(name)
            .spawn(move || {
                let mut worker = Worker {
                    decoder: None,
                    sink: None,
                    decoder_factory,
                    cmd_rx,
                    state: PlayerState::Idle,
                    shared_state: worker_state,
                    shared_position: worker_position,
                    shared_duration: worker_duration,
                    sinks: EventSinks {
                        cx: cx_event_sink,
                        player: player_event_sink,
                    },
                    last_position_emit: Instant::now(),
                    ended_for_this_load: false,
                };
                worker.run();
            })
            .map_err(|e| CantodeError::Internal(format!("spawn worker thread: {e}")))?;

        let handle = Arc::new(PlayerHandle {
            shutdown: Mutex::new(Some(cmd_tx.clone())),
        });
        cx.register(Arc::downgrade(&handle));

        Ok(Self {
            handle,
            join: Mutex::new(Some(join)),
            state,
            position,
            duration,
        })
    }

    /// Load a fresh source. Blocks until the decoder is opened and
    /// metadata is available (or an error occurs). Does not start
    /// playback — call [`Player::play`] afterwards.
    pub fn load(&self, source: Box<dyn AudioSource>) -> crate::Result<Metadata> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Load {
            source,
            reply: tx,
        })?;
        match rx.recv() {
            Ok(LoadResult::Ok(m)) => Ok(m),
            Ok(LoadResult::Err(e)) => Err(e),
            Err(_) => Err(CantodeError::WorkerExited),
        }
    }

    /// Begin or resume playback. No-op if already playing.
    pub fn play(&self) -> crate::Result<()> {
        self.send(Command::Play)
    }

    /// Pause playback. No-op if already paused.
    pub fn pause(&self) -> crate::Result<()> {
        self.send(Command::Pause)
    }

    /// Stop playback and drop the loaded source (back to `Idle`).
    pub fn stop(&self) -> crate::Result<()> {
        self.send(Command::Stop)
    }

    /// Unload the current source without changing transport state if
    /// possible. Equivalent to `stop()` in v1.
    pub fn unload(&self) -> crate::Result<()> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Unload { reply: tx })?;
        rx.recv().map_err(|_| CantodeError::WorkerExited)?
    }

    /// Seek to `target` relative to the source start. Returns the actual
    /// position seeked to.
    pub fn seek(&self, target: Duration) -> crate::Result<Duration> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Seek {
            target,
            reply: tx,
        })?;
        match rx.recv() {
            Ok(SeekResult::Ok(d)) => Ok(d),
            Ok(SeekResult::Err(e)) => Err(e),
            Err(_) => Err(CantodeError::WorkerExited),
        }
    }

    /// Set linear gain. `1.0` is unity; `0.0` is silent.
    pub fn set_volume(&self, vol: f32) -> crate::Result<()> {
        self.send(Command::SetVolume(vol))
    }

    /// Current externally-observable state. Lock-free read.
    pub fn state(&self) -> PlayerState {
        self.state.load()
    }

    /// Current playback position. Lock-free read; updated by the worker
    /// roughly every [`POSITION_EMIT_INTERVAL`].
    pub fn position(&self) -> Duration {
        self.position.load()
    }

    /// Total duration of the loaded source, if known. `None` before the
    /// first `load` or if the container doesn't report it.
    pub fn duration(&self) -> Option<Duration> {
        *self.duration.lock().unwrap()
    }

    // ---- internals ----

    fn send(&self, cmd: Command) -> crate::Result<()> {
        // Acquire the shutdown lock so we never race with our own Drop
        // posting Shutdown; if shutdown already fired, report it.
        let guard = self.handle.shutdown.lock().unwrap();
        match &*guard {
            Some(tx) => tx
                .send(cmd)
                .map_err(|_| CantodeError::WorkerExited),
            None => Err(CantodeError::WorkerExited),
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Best-effort shutdown: clear the shutdown slot, send Shutdown, and
        // join the worker. Ignore all errors — Drop is infallible.
        if let Some(tx) = self.handle.shutdown.lock().unwrap().take() {
            let _ = tx.send(Command::Shutdown);
        }
        if let Some(join) = self.join.lock().unwrap().take() {
            let _ = join.join();
        }
    }
}

// ----- worker -----

struct Worker {
    decoder: Option<Box<dyn Decoder>>,
    sink: Option<Box<dyn crate::output::AudioSink>>,
    decoder_factory: Arc<dyn crate::decoder::DecoderFactory>,
    cmd_rx: mpsc::Receiver<Command>,
    /// Worker-private state. Mirrored into `shared_state` on every change.
    state: PlayerState,
    shared_state: Arc<AtomicState>,
    shared_position: Arc<AtomicPosition>,
    shared_duration: Arc<Mutex<Option<Duration>>>,
    sinks: EventSinks,
    last_position_emit: Instant,
    ended_for_this_load: bool,
}

struct EventSinks {
    cx: Option<Arc<dyn EventSink>>,
    player: Option<Arc<dyn EventSink>>,
}

impl EventSinks {
    fn emit(&self, ev: PlayerEvent) {
        if let Some(s) = &self.cx {
            s.emit(ev.clone());
        }
        if let Some(s) = &self.player {
            s.emit(ev);
        }
    }
}

impl Worker {
    fn run(&mut self) {
        loop {
            // If we're playing, drain work with a short timeout so the
            // decode loop progresses; otherwise block on commands.
            let timeout = if self.state == PlayerState::Playing {
                // Short timeout so we can interleave decode work.
                Duration::from_millis(5)
            } else {
                Duration::from_secs(60 * 60)
            };

            match self.cmd_rx.recv_timeout(timeout) {
                Ok(cmd) => {
                    if self.handle_command(cmd) {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.state == PlayerState::Playing {
                        self.pump_once();
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // All senders dropped (Player::Drop). Bail.
                    return;
                }
            }
        }
    }

    /// Returns `true` if the worker should exit.
    fn handle_command(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Shutdown => return true,
            Command::Load { source, reply } => {
                let result = self.do_load(source);
                let _ = reply.send(match &result {
                    Ok(m) => LoadResult::Ok(m.clone()),
                    Err(e) => LoadResult::Err(e.clone()),
                });
            }
            Command::Play => self.apply_worker_event(WorkerEvent::PlayRequested),
            Command::Pause => self.apply_worker_event(WorkerEvent::PauseRequested),
            Command::Stop => self.do_stop(),
            Command::Unload { reply } => {
                let r = self.do_unload();
                let _ = reply.send(r);
            }
            Command::Seek { target, reply } => {
                let r = self.do_seek(target);
                let _ = reply.send(match r {
                    Ok(d) => SeekResult::Ok(d),
                    Err(e) => SeekResult::Err(e),
                });
            }
            Command::SetVolume(v) => {
                if let Some(s) = self.sink.as_mut() {
                    let _ = s.set_volume(v);
                }
            }
        }
        false
    }

    fn do_load(&mut self, source: Box<dyn AudioSource>) -> crate::Result<Metadata> {
        // Drop any existing decoder/sink first.
        self.teardown_sink();
        self.decoder = None;
        self.ended_for_this_load = false;

        self.apply_worker_event(WorkerEvent::LoadRequested);
        let opened = self.decoder_factory.open(source);
        match opened {
            Ok(dec) => {
                let meta = dec.metadata().clone();
                #[allow(unused_variables)] // only used in the sink-cpal branch
                let fmt = dec.format();
                *self.shared_duration.lock().unwrap() = meta.duration;
                self.decoder = Some(dec);

                // Open the cpal output device. If `sink-cpal` is disabled
                // at compile time there is no output backend — load fails
                // here with a clear error rather than panicking.
                #[cfg(not(feature = "sink-cpal"))]
                {
                    self.apply_worker_event(WorkerEvent::Failed);
                    return Err(CantodeError::Sink(
                        "no sink backend enabled; enable `sink-cpal` to drive output".into(),
                    ));
                }

                #[cfg(feature = "sink-cpal")]
                {
                    let mut sink: Box<dyn crate::output::AudioSink> =
                        Box::new(crate::output::CpalSink::new());
                    match sink.start(fmt) {
                        Ok(_) => {
                            self.sink = Some(sink);
                            self.sinks.emit(PlayerEvent::MetadataReady(meta.clone()));
                            self.apply_worker_event(WorkerEvent::LoadCompleted);
                            Ok(meta)
                        }
                        Err(e) => {
                            self.apply_worker_event(WorkerEvent::Failed);
                            Err(e)
                        }
                    }
                }
            }
            Err(e) => {
                self.apply_worker_event(WorkerEvent::Failed);
                Err(e)
            }
        }
    }

    fn do_stop(&mut self) {
        self.apply_worker_event(WorkerEvent::StopRequested);
        self.teardown_sink();
        self.decoder = None;
        self.shared_position.store(Duration::ZERO);
        *self.shared_duration.lock().unwrap() = None;
        self.ended_for_this_load = false;
    }

    fn do_unload(&mut self) -> crate::Result<()> {
        self.do_stop();
        Ok(())
    }

    fn do_seek(&mut self, target: Duration) -> crate::Result<Duration> {
        let dec = self.decoder.as_mut().ok_or_else(|| {
            CantodeError::InvalidState("seek requires a loaded source".into())
        })?;
        let actual = dec.seek(target)?;
        self.shared_position.store(actual);
        self.sinks.emit(PlayerEvent::PositionChanged(actual));
        // Seeking clears the "ended" latch for this load.
        self.ended_for_this_load = false;
        Ok(actual)
    }

    /// Decode one frame and push it to the sink. Called from the
    /// Playing-loop body.
    fn pump_once(&mut self) {
        let Some(dec) = self.decoder.as_mut() else {
            return;
        };
        match dec.next_frame() {
            Ok(Some(frame)) => {
                self.shared_position.store(frame.timestamp);
                if let Some(s) = self.sink.as_mut() {
                    let _ = s.write(&frame.data);
                }
                self.maybe_emit_position();
            }
            Ok(None) => {
                if !self.ended_for_this_load {
                    self.ended_for_this_load = true;
                    self.sinks.emit(PlayerEvent::Ended);
                }
                self.apply_worker_event(WorkerEvent::EndOfStream);
            }
            Err(_e) => {
                // Non-fatal: skip. A future improvement is to surface
                // repeated decode failures via PlayerEvent::Error.
                tracing::debug!("decode error in pump_once; skipping frame");
            }
        }
    }

    fn maybe_emit_position(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_position_emit) >= POSITION_EMIT_INTERVAL {
            self.last_position_emit = now;
            self.sinks
                .emit(PlayerEvent::PositionChanged(self.shared_position.load()));
        }
    }

    fn apply_worker_event(&mut self, ev: WorkerEvent) {
        match transition(self.state, ev) {
            Ok(next) => {
                if next != self.state {
                    self.state = next;
                    self.shared_state.store(next);
                    self.sinks.emit(PlayerEvent::StateChanged(next));
                    self.on_state_changed(next);
                }
            }
            Err(e) => {
                // Illegal transition — surface as a non-fatal error event.
                self.sinks.emit(PlayerEvent::Error(e));
            }
        }
    }

    /// Side effects to run after a successful transition.
    fn on_state_changed(&mut self, next: PlayerState) {
        match next {
            PlayerState::Playing => {
                if let Some(s) = self.sink.as_mut() {
                    let _ = s.resume();
                }
            }
            PlayerState::Paused => {
                if let Some(s) = self.sink.as_mut() {
                    let _ = s.pause();
                }
            }
            PlayerState::Ended => {
                // Pause the sink so the device stops outputting whatever
                // tail samples it had buffered.
                if let Some(s) = self.sink.as_mut() {
                    let _ = s.pause();
                }
            }
            PlayerState::Idle => {
                // No sink side-effect; teardown happens in `do_stop`.
            }
            _ => {}
        }
    }

    fn teardown_sink(&mut self) {
        if let Some(mut s) = self.sink.take() {
            let _ = s.stop();
        }
    }
}

// ----- shared atomic state -----

struct AtomicState {
    bits: AtomicU64,
}

impl AtomicState {
    fn new(initial: PlayerState) -> Self {
        Self {
            bits: AtomicU64::new(initial as u64),
        }
    }
    fn store(&self, s: PlayerState) {
        self.bits.store(s as u64, Ordering::Relaxed);
    }
    fn load(&self) -> PlayerState {
        // Safety: the stored value is always one of the valid enum
        // discriminants (we only ever `store` a `PlayerState`).
        match self.bits.load(Ordering::Relaxed) {
            0 => PlayerState::Idle,
            1 => PlayerState::Loading,
            2 => PlayerState::Paused,
            3 => PlayerState::Playing,
            4 => PlayerState::Buffering,
            5 => PlayerState::Ended,
            6 => PlayerState::Error,
            _ => PlayerState::Error,
        }
    }
}

struct AtomicPosition {
    nanos: AtomicU64,
}

impl AtomicPosition {
    fn new() -> Self {
        Self {
            nanos: AtomicU64::new(0),
        }
    }
    fn store(&self, d: Duration) {
        self.nanos.store(d.as_nanos() as u64, Ordering::Relaxed);
    }
    fn load(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::Relaxed))
    }
}
