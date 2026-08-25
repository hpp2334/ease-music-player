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
//!
//! The worker tracks its progress in a data-carrying `Phase` enum
//! (private to this module): every variant that has a live decode session
//! carries the `Loaded` session (decoder + sink + conversion state)
//! directly, so "sink open ⟹ decoder open" holds by construction instead
//! of by discipline, and session teardown (sink stop + observable resets)
//! happens in exactly one place (`Loaded::drop`). The public, data-free
//! `PlayerState` is *derived* from `Phase` and mirrored into a small
//! `SharedStatus` register that the `Player` handle reads lock-free from
//! other threads — `Phase` itself never leaves the worker (its payloads
//! are `Send` but deliberately not `Sync`).

use std::{
    fmt, mem,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    AudioSource, CantodeError, Metadata,
    context::{PlayerContext, PlayerHandle},
    decoder::{DecodedFrame, Decoder},
    events::{EventSink, PlayerEvent},
    output::AudioSinkFactory,
    state::{PlayerState, WorkerEvent, transition},
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
///
/// Acts as a builder: start from [`PlayerConfig::default`] and chain the
/// setters, then pass to [`Player::with_config`]. All fields are also
/// public, so struct-update syntax works too.
#[derive(Default)]
pub struct PlayerConfig {
    /// Optional per-player event sink, in addition to the
    /// [`PlayerContext`]'s global one.
    pub event_sink: Option<Arc<dyn EventSink>>,
    /// Optional replacement for the default (cpal device) audio sink.
    /// The factory is called once per loaded source on the worker thread.
    ///
    /// Unset by default. See [`AudioSinkFactory`].
    pub audio_sink_factory: Option<AudioSinkFactory>,
}

impl PlayerConfig {
    /// Set (or clear, with `None`) the per-player event sink.
    #[must_use]
    pub fn event_sink(mut self, sink: Option<Arc<dyn EventSink>>) -> Self {
        self.event_sink = sink;
        self
    }

    /// Replace the default cpal device sink with a custom one. The
    /// factory is called once per loaded source on the worker thread.
    #[must_use]
    pub fn audio_sink_factory(mut self, factory: AudioSinkFactory) -> Self {
        self.audio_sink_factory = Some(factory);
        self
    }
}

/// A handle to one audio playback pipeline.
///
/// Created via [`Player::new`]; owns one worker thread for its whole
/// lifetime. Dropping a `Player` posts a shutdown command and joins the
/// worker. All public methods are non-blocking.
pub struct Player {
    handle: Arc<PlayerHandle>,
    join: Mutex<Option<JoinHandle<()>>>,
    /// Lock-free view of the worker's progress. See `SharedStatus` for
    /// why this is a projection rather than shared ownership of the
    /// worker's `Phase`.
    shared: Arc<SharedStatus>,
}

impl Player {
    /// Create a new player attached to `cx`.
    ///
    /// Spawns a dedicated worker thread named `cantode-player-N` and
    /// registers the player in `cx`'s live-player registry.
    pub fn new(cx: &PlayerContext) -> Result<Self, CantodeError> {
        Self::with_config(cx, PlayerConfig::default())
    }

    /// Like [`Player::new`] but with per-player overrides.
    pub fn with_config(cx: &PlayerContext, config: PlayerConfig) -> Result<Self, CantodeError> {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel(COMMAND_CHANNEL_CAP);
        let shared = Arc::new(SharedStatus::new());

        let decoder_factory = Arc::clone(cx.decoder_factory());
        let cx_event_sink = cx.event_sink().cloned();
        let player_event_sink = config.event_sink.clone();
        // Resolve the sink factory once, on the handle side: custom if
        // configured, otherwise the default cpal device sink. The worker
        // calls it at the top of every `load`.
        let sink_factory: AudioSinkFactory = config
            .audio_sink_factory
            .unwrap_or_else(|| Arc::new(|| Ok(Box::new(crate::output::CpalSink::new()))));

        let worker_shared = Arc::clone(&shared);

        let name = cx.next_worker_name();
        let join = thread::Builder::new()
            .name(name)
            .spawn(move || {
                let mut worker = Worker {
                    phase: Phase::Idle,
                    decoder_factory,
                    cmd_rx,
                    shared: worker_shared,
                    sink_factory,
                    sinks: EventSinks {
                        cx: cx_event_sink,
                        player: player_event_sink,
                    },
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
            shared,
        })
    }

    /// Load a fresh source. Blocks until the decoder is opened and
    /// metadata is available (or an error occurs). Does not start
    /// playback — call [`Player::play`] afterwards.
    pub fn load(&self, source: Box<dyn AudioSource>) -> crate::Result<Metadata> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Load { source, reply: tx })?;
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
        self.send(Command::Seek { target, reply: tx })?;
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
        self.shared.state.load()
    }

    /// Current playback position. Lock-free read; updated by the worker
    /// roughly every [`POSITION_EMIT_INTERVAL`].
    pub fn position(&self) -> Duration {
        self.shared.position.load()
    }

    /// Total duration of the loaded source, if known. `None` before the
    /// first `load` or if the container doesn't report it.
    pub fn duration(&self) -> Option<Duration> {
        *self.shared.duration.lock().unwrap()
    }

    // ---- internals ----

    fn send(&self, cmd: Command) -> crate::Result<()> {
        // Acquire the shutdown lock so we never race with our own Drop
        // posting Shutdown; if shutdown already fired, report it.
        let guard = self.handle.shutdown.lock().unwrap();
        match &*guard {
            Some(tx) => tx.send(cmd).map_err(|_| CantodeError::WorkerExited),
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

/// The observable projection of the worker's `Phase`.
///
/// The worker is the sole writer; the `Player` handle reads these
/// lock-free from other threads. This is deliberately *not* part of
/// `Phase`: `Phase` owns `!Sync` resources (decoder, sink) that must
/// never leave the worker thread, while these status registers must stay
/// readable at any moment — including while the worker is mid-`load` or
/// blocked in a backpressured sink write, and in the payload-less phases
/// (`Idle` / `Loading` / `Error`) where there is no session to hang data
/// on.
struct SharedStatus {
    /// Mirrored by `Worker::set_phase` — its only writer.
    state: AtomicState,
    /// Last decoded-frame timestamp; reset by `Loaded::drop`.
    position: AtomicPosition,
    /// Duration of the loaded source; set by `do_load` on success, reset
    /// by `Loaded::drop`.
    duration: Mutex<Option<Duration>>,
}

impl SharedStatus {
    fn new() -> Self {
        Self {
            state: AtomicState::new(PlayerState::Idle),
            position: AtomicPosition::new(),
            duration: Mutex::new(None),
        }
    }
}

/// Everything that exists only while a source is loaded and its sink is
/// open: one decode session. Moves as a unit between the payload-carrying
/// `Phase` variants (`Paused` ↔ `Playing` ↔ `Buffering` → `Ended`).
struct Loaded {
    decoder: Box<dyn Decoder>,
    sink: Box<dyn crate::output::AudioSink>,
    /// Channel count of the currently-loaded source, captured in `do_load`.
    /// Zero means "no source loaded" / "no conversion needed yet".
    src_channels: u16,
    /// Channel count the device stream actually opened with, captured in
    /// `do_load` from the format returned by `sink.start`. When this
    /// differs from `src_channels`, `write_frame` runs each decoded frame
    /// through `remux_channels` before writing it to the sink. The
    /// mismatch is rare but real — see `CpalSink::start` /
    /// `pick_supported_config`.
    device_channels: u16,
    /// Reusable scratch buffer for the channel-conversion path. Empty when
    /// `src_channels == device_channels`.
    remix_buf: Vec<f32>,
    /// Latch: `PlayerEvent::Ended` has been emitted for this load already.
    ended: bool,
    /// Clone of the worker's shared status, so the session-scoped
    /// observables (position, duration) reset exactly when the session
    /// dies — wherever that happens from.
    shared: Arc<SharedStatus>,
}

impl Loaded {
    /// Push one decoded frame to the sink, converting the channel layout
    /// when the device insisted on a count other than the source's. The
    /// common case (`src_channels == device_channels`) skips the scratch
    /// buffer entirely.
    fn write_frame(&mut self, frame: &DecodedFrame) {
        if self.src_channels != 0
            && self.device_channels != 0
            && self.src_channels != self.device_channels
        {
            let n_frames = frame.data.len() / self.src_channels as usize;
            let out_samples = n_frames * self.device_channels as usize;
            if self.remix_buf.len() < out_samples {
                self.remix_buf.resize(out_samples, 0.0);
            }
            let written = crate::output::remux_channels(
                &frame.data,
                self.src_channels,
                &mut self.remix_buf[..out_samples],
                self.device_channels,
            );
            let _ = self.sink.write(&self.remix_buf[..written]);
        } else {
            let _ = self.sink.write(&frame.data);
        }
    }
}

impl Drop for Loaded {
    fn drop(&mut self) {
        // Session teardown in exactly one place. Variant-to-variant moves
        // (which destructure the session out rather than dropping it)
        // never run this — only true session exit does: stop, replace by
        // a new load, `Failed`, or worker teardown.
        let _ = self.sink.stop();
        self.shared.position.store(Duration::ZERO);
        *self.shared.duration.lock().unwrap() = None;
    }
}

/// Worker-side state machine state: the data-carrying twin of the public
/// [`PlayerState`]. Every variant with a live decode session carries its
/// [`Loaded`] payload, so the invariants "sink open ⟹ decoder open" and
/// "conversion state exists ⟺ session exists" hold by construction.
///
/// `Phase` is worker-private (its payloads are `!Sync`); the public state
/// is the projection [`Phase::state`], mirrored into `SharedStatus` by
/// `Worker::set_phase`. Transitions are still decided by the pure
/// [`transition`] function over the projected state; `morph` then
/// reshapes the payload to match.
enum Phase {
    Idle,
    Loading,
    Paused(Loaded),
    Playing {
        loaded: Loaded,
        /// Throttle timestamp for `PlayerEvent::PositionChanged`; only
        /// consulted while playing. Reset when (re)entering `Playing`.
        last_position_emit: Instant,
    },
    Buffering(Loaded),
    Ended(Loaded),
    /// Wedged. Any session it held was dropped; recovery is a fresh
    /// `load` or `stop`.
    Error,
}

impl Phase {
    /// The externally-observable state this phase projects to.
    fn state(&self) -> PlayerState {
        match self {
            Phase::Idle => PlayerState::Idle,
            Phase::Loading => PlayerState::Loading,
            Phase::Paused(_) => PlayerState::Paused,
            Phase::Playing { .. } => PlayerState::Playing,
            Phase::Buffering(_) => PlayerState::Buffering,
            Phase::Ended(_) => PlayerState::Ended,
            Phase::Error => PlayerState::Error,
        }
    }

    /// The live session, if this phase has one.
    fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        match self {
            Phase::Paused(loaded)
            | Phase::Playing { loaded, .. }
            | Phase::Buffering(loaded)
            | Phase::Ended(loaded) => Some(loaded),
            Phase::Idle | Phase::Loading | Phase::Error => None,
        }
    }
}

impl fmt::Debug for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Payloads are trait objects; project to the public state.
        write!(f, "Phase::{:?}", self.state())
    }
}

/// Reshape `current` into the phase shape for `next`, moving the `Loaded`
/// session between variants and dropping it on exit from the loaded
/// phases (the drop stops the sink and resets position/duration).
///
/// Only called when `next != current.state()` — no-op transitions never
/// morph — so the `unreachable!` arms cannot fire unless the
/// [`transition`] table or the call sites drift. `Paused` is reached only
/// by `do_load`'s inline completion (from `Loading`, which carries no
/// session), never through `morph`.
fn morph(current: Phase, next: PlayerState) -> Phase {
    match next {
        // Session exits: `current` — with its session, if any — drops
        // here (`Loaded::drop` runs).
        PlayerState::Idle => Phase::Idle,
        PlayerState::Loading => Phase::Loading,
        PlayerState::Error => Phase::Error,
        PlayerState::Playing => {
            let loaded = match current {
                Phase::Paused(loaded) | Phase::Buffering(loaded) => loaded,
                // Defensive: no-op transitions never reach `morph`, but
                // keep the session rather than panic if that drifts.
                Phase::Playing { loaded, .. } | Phase::Ended(loaded) => loaded,
                phase @ (Phase::Idle | Phase::Loading | Phase::Error) => {
                    unreachable!("cannot morph {phase:?} into Playing")
                }
            };
            Phase::Playing {
                loaded,
                last_position_emit: Instant::now(),
            }
        }
        PlayerState::Buffering => {
            let loaded = match current {
                Phase::Playing { loaded, .. } => loaded,
                phase => unreachable!("cannot morph {phase:?} into Buffering"),
            };
            Phase::Buffering(loaded)
        }
        PlayerState::Paused => {
            let loaded = match current {
                Phase::Playing { loaded, .. } | Phase::Buffering(loaded) => loaded,
                phase => unreachable!("cannot morph {phase:?} into Paused"),
            };
            Phase::Paused(loaded)
        }
        PlayerState::Ended => {
            let loaded = match current {
                Phase::Playing { loaded, .. }
                | Phase::Paused(loaded)
                | Phase::Buffering(loaded) => loaded,
                phase => unreachable!("cannot morph {phase:?} into Ended"),
            };
            Phase::Ended(loaded)
        }
    }
}

struct Worker {
    /// Worker-private state machine state; the single source of truth for
    /// what the player is doing and what it owns. Projected into
    /// `shared.state` by `set_phase` — the mirror's only writer.
    phase: Phase,
    decoder_factory: Arc<dyn crate::decoder::DecoderFactory>,
    cmd_rx: mpsc::Receiver<Command>,
    /// Constructs the sink for each loaded source (custom, or the default
    /// cpal device sink when the config didn't provide one).
    sink_factory: AudioSinkFactory,
    /// Observable projection shared with the `Player` handle.
    shared: Arc<SharedStatus>,
    sinks: EventSinks,
}

/// What one `pump_once` iteration produced. Split out so the decoder/sink
/// work happens under the `Phase::Playing` borrow while event emission
/// happens after it ends.
enum PumpOutcome {
    /// A frame was decoded and written; `emit` says the position-event
    /// throttle elapsed for it.
    Frame { position: Duration, emit: bool },
    /// The decoder reached end of stream.
    EndOfStream,
    /// A (non-fatal) decode error; the frame was skipped.
    Skipped,
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
            let timeout = if matches!(self.phase, Phase::Playing { .. }) {
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
                    if matches!(self.phase, Phase::Playing { .. }) {
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
                if let Some(loaded) = self.phase.loaded_mut() {
                    let _ = loaded.sink.set_volume(v);
                }
            }
        }
        false
    }

    fn do_load(&mut self, source: Box<dyn AudioSource>) -> crate::Result<Metadata> {
        // Drop any existing session first: the old sink stops and the
        // session-scoped observables reset via `Loaded::drop`, before the
        // potentially slow open below. Assigning `Idle` directly (not via
        // `set_phase`) keeps the shared mirror at the old state until the
        // `LoadRequested` transition publishes `Loading` — the same
        // observable sequence as before this refactor.
        self.phase = Phase::Idle;

        self.apply_worker_event(WorkerEvent::LoadRequested);
        let opened = self.decoder_factory.open(source);
        let dec = match opened {
            Ok(dec) => dec,
            Err(e) => {
                self.apply_worker_event(WorkerEvent::Failed);
                return Err(e);
            }
        };
        let meta = dec.metadata().clone();
        let fmt = dec.format();

        // Construct the sink (custom factory, or the default cpal device)
        // and open it. Both failure paths mirror each other: transition to
        // `Error` and propagate.
        let mut sink: Box<dyn crate::output::AudioSink> = match (self.sink_factory)() {
            Ok(sink) => sink,
            Err(e) => {
                self.apply_worker_event(WorkerEvent::Failed);
                return Err(e);
            }
        };
        let actual_fmt = match sink.start(fmt) {
            Ok(actual_fmt) => actual_fmt,
            Err(e) => {
                self.apply_worker_event(WorkerEvent::Failed);
                return Err(e);
            }
        };

        let loaded = Loaded {
            decoder: dec,
            sink,
            // Capture the channel counts so `write_frame` can convert when
            // the device insisted on a channel count other than the
            // source's.
            src_channels: fmt.channels,
            device_channels: actual_fmt.channels,
            remix_buf: Vec::new(),
            ended: false,
            shared: Arc::clone(&self.shared),
        };
        if loaded.src_channels != loaded.device_channels {
            tracing::info!(
                src = loaded.src_channels,
                device = loaded.device_channels,
                "device channel count differs from source; \
                 enabling channel conversion in worker"
            );
        }

        // Publish metadata/duration only once the sink is up (the success
        // path): a failed load leaves the observables cleared by the old
        // session's drop, rather than half-updated.
        *self.shared.duration.lock().unwrap() = meta.duration;
        self.sinks.emit(PlayerEvent::MetadataReady(meta.clone()));

        // Inline completion `Loading → Paused`: the only transition that
        // carries a *fresh* session, which is why it bypasses `morph`
        // (`Loading` has no payload to reshape). Still routed through
        // `transition` so the table stays the single authority on what a
        // completed load means. `do_load` runs synchronously on the worker
        // thread, so no command can interleave and no pending-play
        // bookkeeping is needed.
        debug_assert!(matches!(self.phase, Phase::Loading));
        let next = transition(PlayerState::Loading, WorkerEvent::LoadCompleted)
            .expect("LoadCompleted is legal from Loading");
        self.set_phase(Phase::Paused(loaded));
        debug_assert_eq!(next, PlayerState::Paused);
        Ok(meta)
    }

    fn do_stop(&mut self) {
        // `StopRequested → Idle`. `morph` drops the session (if any),
        // which stops the sink and resets position/duration. From `Idle`
        // the transition is a no-op and nothing runs.
        self.apply_worker_event(WorkerEvent::StopRequested);
    }

    fn do_unload(&mut self) -> crate::Result<()> {
        self.do_stop();
        Ok(())
    }

    fn do_seek(&mut self, target: Duration) -> crate::Result<Duration> {
        let Some(loaded) = self.phase.loaded_mut() else {
            return Err(CantodeError::InvalidState(
                "seek requires a loaded source".into(),
            ));
        };
        let actual = loaded.decoder.seek(target)?;
        // Flush the sink's buffered audio: it contains up to `buffer_secs`
        // of pre-seek samples that would otherwise play out before the new
        // seek position's audio arrives. Without this flush, the listener
        // hears ~2s of stale audio mixed with the new position's samples.
        let _ = loaded.sink.flush();
        // Seeking clears the "ended" latch for this load.
        loaded.ended = false;
        self.shared.position.store(actual);
        self.sinks.emit(PlayerEvent::PositionChanged(actual));
        Ok(actual)
    }

    /// Decode one frame and push it to the sink. Called from the
    /// Playing-loop body.
    fn pump_once(&mut self) {
        // Stage 1 — decoder + sink work under the `Playing` borrow.
        // (`self.shared` is a disjoint field, so the position store below
        // is fine while the borrow is live.)
        let outcome = {
            let Phase::Playing {
                loaded,
                last_position_emit,
            } = &mut self.phase
            else {
                return;
            };
            match loaded.decoder.next_frame() {
                Ok(Some(frame)) => {
                    self.shared.position.store(frame.timestamp);
                    loaded.write_frame(&frame);
                    let now = Instant::now();
                    let emit = now.duration_since(*last_position_emit) >= POSITION_EMIT_INTERVAL;
                    if emit {
                        *last_position_emit = now;
                    }
                    PumpOutcome::Frame {
                        position: frame.timestamp,
                        emit,
                    }
                }
                Ok(None) => PumpOutcome::EndOfStream,
                Err(_e) => {
                    // Non-fatal: skip. A future improvement is to surface
                    // repeated decode failures via PlayerEvent::Error.
                    tracing::debug!("decode error in pump_once; skipping frame");
                    PumpOutcome::Skipped
                }
            }
        };

        // Stage 2 — events and transitions, after the borrow ended.
        match outcome {
            PumpOutcome::Frame { position, emit } => {
                if emit {
                    self.sinks.emit(PlayerEvent::PositionChanged(position));
                }
            }
            PumpOutcome::EndOfStream => {
                if let Some(loaded) = self.phase.loaded_mut()
                    && !loaded.ended
                {
                    loaded.ended = true;
                    self.sinks.emit(PlayerEvent::Ended);
                }
                self.apply_worker_event(WorkerEvent::EndOfStream);
            }
            PumpOutcome::Skipped => {}
        }
    }

    fn apply_worker_event(&mut self, ev: WorkerEvent) {
        let from = self.phase.state();
        match transition(from, ev) {
            Ok(next) => {
                if next != from {
                    // Take the current phase out, reshape it to the new
                    // state's shape (moving or dropping the session), and
                    // commit through `set_phase`.
                    let current = mem::replace(&mut self.phase, Phase::Idle);
                    self.set_phase(morph(current, next));
                }
            }
            Err(e) => {
                // Illegal transition — surface as a non-fatal error event.
                self.sinks.emit(PlayerEvent::Error(e));
            }
        }
    }

    /// Commit a new phase. The single writer of the shared state mirror:
    /// the mirror can never disagree with `self.phase`.
    fn set_phase(&mut self, phase: Phase) {
        let state = phase.state();
        self.phase = phase;
        self.shared.state.store(state);
        self.sinks.emit(PlayerEvent::StateChanged(state));
        self.on_phase_changed();
    }

    /// Side effects to run after a phase change.
    fn on_phase_changed(&mut self) {
        match &mut self.phase {
            Phase::Playing { loaded, .. } => {
                let _ = loaded.sink.resume();
            }
            Phase::Paused(loaded) | Phase::Ended(loaded) => {
                // Pause the sink so the device stops outputting whatever
                // samples it had buffered (on `Ended`, the tail of the
                // source).
                let _ = loaded.sink.pause();
            }
            // `Buffering` keeps the stream running on purpose: the sink
            // ring buffer drains what it has while the source stalls.
            Phase::Buffering(_) | Phase::Idle | Phase::Loading | Phase::Error => {}
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

#[cfg(test)]
mod tests {
    //! Unit tests for the worker's phase machinery — `Phase`, `morph`,
    //! `Loaded`, and the session lifecycle — using stub decoders/sinks so
    //! they need no audio device. End-to-end behavior is covered by
    //! `tests/` against the real cpal host.

    use super::*;
    use crate::decoder::{AudioFormat, Decoder, DecoderFactory};
    use crate::output::AudioSink;
    use std::sync::Mutex;

    /// Decoder stub: reports a fixed format, yields no frames.
    struct StubDecoder {
        fmt: AudioFormat,
    }

    impl Decoder for StubDecoder {
        fn next_frame(&mut self) -> crate::Result<Option<DecodedFrame>> {
            Ok(None)
        }
        fn seek(&mut self, target: Duration) -> crate::Result<Duration> {
            Ok(target)
        }
        fn format(&self) -> AudioFormat {
            self.fmt
        }
        fn metadata(&self) -> &Metadata {
            // The worker never reads metadata from the stub (do_load is
            // not exercised here); a static default is enough.
            static META: Metadata = Metadata {
                format: AudioFormat::new(0, 0),
                duration: None,
                total_samples: None,
                tags: Vec::new(),
                cover_art: None,
            };
            &META
        }
    }

    /// Decoder factory stub — present so a `Worker` can be constructed;
    /// these tests never call `do_load`.
    struct StubFactory;

    impl DecoderFactory for StubFactory {
        fn open(&self, _source: Box<dyn AudioSource>) -> crate::Result<Box<dyn Decoder>> {
            Err(CantodeError::Internal("stub factory".into()))
        }
    }

    /// Sink stub: records every call name and every written sample.
    #[derive(Clone, Default)]
    struct SinkLog {
        calls: Arc<Mutex<Vec<String>>>,
        samples: Arc<Mutex<Vec<f32>>>,
    }

    impl SinkLog {
        fn record(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_string());
        }
        fn recorded(&self, call: &str) -> bool {
            self.calls.lock().unwrap().iter().any(|c| c == call)
        }
    }

    struct StubSink {
        log: SinkLog,
    }

    impl AudioSink for StubSink {
        fn start(&mut self, _fmt: AudioFormat) -> crate::Result<AudioFormat> {
            self.log.record("start");
            Ok(AudioFormat::new(2, 48_000))
        }
        fn stop(&mut self) -> crate::Result<()> {
            self.log.record("stop");
            Ok(())
        }
        fn write(&mut self, frames: &[f32]) -> crate::Result<()> {
            self.log.samples.lock().unwrap().extend_from_slice(frames);
            Ok(())
        }
        fn flush(&mut self) -> crate::Result<()> {
            self.log.record("flush");
            Ok(())
        }
        fn pause(&mut self) -> crate::Result<()> {
            self.log.record("pause");
            Ok(())
        }
        fn resume(&mut self) -> crate::Result<()> {
            self.log.record("resume");
            Ok(())
        }
        fn set_volume(&mut self, _vol: f32) -> crate::Result<()> {
            Ok(())
        }
        fn latency(&self) -> Duration {
            Duration::ZERO
        }
    }

    /// A `Loaded` session backed by stubs, plus the handles to observe it.
    struct Fixture {
        log: SinkLog,
        shared: Arc<SharedStatus>,
    }

    fn loaded_session(src: u16, device: u16) -> (Loaded, Fixture) {
        let log = SinkLog::default();
        let shared = Arc::new(SharedStatus::new());
        let loaded = Loaded {
            decoder: Box::new(StubDecoder {
                fmt: AudioFormat::new(src, 48_000),
            }),
            sink: Box::new(StubSink { log: log.clone() }),
            src_channels: src,
            device_channels: device,
            remix_buf: Vec::new(),
            ended: false,
            shared: Arc::clone(&shared),
        };
        (loaded, Fixture { log, shared })
    }

    fn worker_with(phase: Phase, shared: Arc<SharedStatus>) -> Worker {
        let (_tx, rx) = mpsc::channel();
        Worker {
            phase,
            decoder_factory: Arc::new(StubFactory),
            cmd_rx: rx,
            sink_factory: Arc::new(|| Err(CantodeError::Internal("stub factory".into()))),
            shared,
            sinks: EventSinks {
                cx: None,
                player: None,
            },
        }
    }

    #[test]
    fn phase_projects_to_player_state() {
        let (loaded, _fx) = loaded_session(2, 2);
        assert_eq!(Phase::Idle.state(), PlayerState::Idle);
        assert_eq!(Phase::Loading.state(), PlayerState::Loading);
        assert_eq!(Phase::Paused(loaded).state(), PlayerState::Paused);
        let (loaded, _fx) = loaded_session(2, 2);
        assert_eq!(
            Phase::Playing {
                loaded,
                last_position_emit: Instant::now()
            }
            .state(),
            PlayerState::Playing
        );
        let (loaded, _fx) = loaded_session(2, 2);
        assert_eq!(Phase::Buffering(loaded).state(), PlayerState::Buffering);
        let (loaded, _fx) = loaded_session(2, 2);
        assert_eq!(Phase::Ended(loaded).state(), PlayerState::Ended);
        assert_eq!(Phase::Error.state(), PlayerState::Error);
    }

    #[test]
    fn morph_moves_the_session_between_loaded_variants() {
        // Paused → Playing (PlayRequested).
        let (loaded, fx) = loaded_session(2, 2);
        let mut phase = morph(Phase::Paused(loaded), PlayerState::Playing);
        assert!(matches!(phase, Phase::Playing { .. }));
        // Playing → Paused (PauseRequested).
        phase = morph(phase, PlayerState::Paused);
        assert!(matches!(phase, Phase::Paused(_)));
        // Paused → Playing → Buffering (BufferUnderrun).
        phase = morph(phase, PlayerState::Playing);
        phase = morph(phase, PlayerState::Buffering);
        assert!(matches!(phase, Phase::Buffering(_)));
        // Buffering → Ended (EndOfStream).
        phase = morph(phase, PlayerState::Ended);
        assert!(matches!(phase, Phase::Ended(_)));

        // The session survived every move: it's still the same sink (the
        // log is shared), and it was never stopped or reset.
        phase
            .loaded_mut()
            .expect("Ended carries the session")
            .sink
            .write(&[0.5])
            .unwrap();
        assert_eq!(fx.log.samples.lock().unwrap().last(), Some(&0.5));
        assert!(!fx.log.recorded("stop"));
    }

    #[test]
    fn morph_moves_do_not_reset_observables() {
        // Position/duration reset only on session *exit*, never on a
        // variant-to-variant move (which destructures, not drops).
        let (loaded, fx) = loaded_session(2, 2);
        fx.shared.position.store(Duration::from_secs(42));
        *fx.shared.duration.lock().unwrap() = Some(Duration::from_secs(60));

        let phase = morph(Phase::Paused(loaded), PlayerState::Playing);
        let phase = morph(phase, PlayerState::Buffering);
        let _phase = morph(phase, PlayerState::Ended);

        assert_eq!(fx.shared.position.load(), Duration::from_secs(42));
        assert_eq!(
            *fx.shared.duration.lock().unwrap(),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn morph_drops_the_session_on_unloaded_targets() {
        for target in [PlayerState::Idle, PlayerState::Loading, PlayerState::Error] {
            let (loaded, fx) = loaded_session(2, 2);
            fx.shared.position.store(Duration::from_secs(7));
            *fx.shared.duration.lock().unwrap() = Some(Duration::from_secs(7));

            let phase = morph(Phase::Paused(loaded), target);
            assert_eq!(phase.state(), target);

            // The drop ran: sink stopped, observables reset.
            assert!(fx.log.recorded("stop"), "sink not stopped for {target:?}");
            assert_eq!(fx.shared.position.load(), Duration::ZERO);
            assert_eq!(*fx.shared.duration.lock().unwrap(), None);
        }
    }

    #[test]
    fn loaded_drop_is_the_single_teardown_point() {
        let (loaded, fx) = loaded_session(2, 2);
        fx.shared.position.store(Duration::from_secs(1));
        *fx.shared.duration.lock().unwrap() = Some(Duration::from_secs(1));
        drop(loaded);
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position.load(), Duration::ZERO);
        assert_eq!(*fx.shared.duration.lock().unwrap(), None);
    }

    #[test]
    fn write_frame_passes_through_when_channels_match() {
        let (mut loaded, fx) = loaded_session(2, 2);
        let frame = DecodedFrame {
            data: vec![0.1, 0.2, 0.3, 0.4],
            frames: 2,
            timestamp: Duration::ZERO,
        };
        loaded.write_frame(&frame);
        assert_eq!(*fx.log.samples.lock().unwrap(), frame.data);
        assert!(loaded.remix_buf.is_empty());
    }

    #[test]
    fn write_frame_remuxes_when_channels_differ() {
        let (mut loaded, fx) = loaded_session(2, 1);
        let frame = DecodedFrame {
            // (0.2, 0.6) → 0.4 ; (1.0, -1.0) → 0.0
            data: vec![0.2, 0.6, 1.0, -1.0],
            frames: 2,
            timestamp: Duration::ZERO,
        };
        loaded.write_frame(&frame);
        let samples = fx.log.samples.lock().unwrap().clone();
        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.4).abs() < 1e-6);
        assert!(samples[1].abs() < 1e-6);
    }

    #[test]
    fn seek_requires_a_session() {
        let mut worker = worker_with(Phase::Idle, Arc::new(SharedStatus::new()));
        let err = worker.do_seek(Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, CantodeError::InvalidState(_)));
    }

    #[test]
    fn seek_flushes_the_sink_and_clears_the_ended_latch() {
        let (loaded, fx) = loaded_session(2, 2);
        let shared = Arc::clone(&fx.shared);
        let mut worker = worker_with(Phase::Paused(loaded), shared);

        // Simulate an already-ended session, then seek back into it.
        worker.phase.loaded_mut().unwrap().ended = true;
        let actual = worker.do_seek(Duration::from_secs(5)).unwrap();

        assert_eq!(actual, Duration::from_secs(5));
        assert!(fx.log.recorded("flush"));
        assert!(!worker.phase.loaded_mut().unwrap().ended);
        assert_eq!(fx.shared.position.load(), Duration::from_secs(5));
    }

    #[test]
    fn stop_tears_the_session_down_and_resets_observables() {
        let (loaded, fx) = loaded_session(2, 2);
        let shared = Arc::clone(&fx.shared);
        let mut worker = worker_with(Phase::Paused(loaded), shared);
        fx.shared.position.store(Duration::from_secs(9));
        *fx.shared.duration.lock().unwrap() = Some(Duration::from_secs(9));

        worker.do_stop();

        assert!(matches!(worker.phase, Phase::Idle));
        assert_eq!(worker.shared.state.load(), PlayerState::Idle);
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position.load(), Duration::ZERO);
        assert_eq!(*fx.shared.duration.lock().unwrap(), None);
    }
}
