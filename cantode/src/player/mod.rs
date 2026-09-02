//! [`Player`]: the public orchestrator and its worker thread.
//!
//! Each `Player` owns one dedicated worker thread that runs the
//! decode→sink loop. The public API methods are synchronous and
//! non-blocking: they post commands onto an mpsc channel and return.
//! The worker drains commands, drives the state machine via the pure
//! `transition` table, and emits `PlayerEvent`s to the configured sinks.
//!
//! This module is a tree, one concept per file:
//!
//! - `command` — the handle→worker protocol: `Command`, the `Load` /
//!   `Seek` reply envelopes, and the command-channel capacity.
//! - `worker` — the event loop (`recv_timeout` cadence) and the
//!   operations it dispatches to (`do_load`, `do_seek`, `pump_once`, …).
//! - `phase` — the worker-side, data-carrying state machine shapes
//!   (`Phase`, `morph`) around the pure transition table in
//!   [`crate::state`].
//! - `session` — `Loaded`: one decode session (decoder + sink +
//!   conversion state) with single-point teardown.
//! - `shared` — `SharedStatus`: the lock-free observable projection the
//!   `Player` handle reads from other threads.
//!
//! The worker tracks its progress in a data-carrying `Phase` enum
//! (private to this tree): every variant that has a live decode session
//! carries the `Loaded` session (decoder + sink + conversion state)
//! directly, so "sink open ⟹ decoder open" holds by construction instead
//! of by discipline, and session teardown happens in exactly one place
//! (`Loaded::drop`). The public, data-free [`PlayerState`] is *derived*
//! from `Phase` and mirrored into a small `SharedStatus` register that
//! the `Player` handle reads lock-free from other threads — `Phase`
//! itself never leaves the worker (its payloads are `Send` but
//! deliberately not `Sync`).

mod command;
mod phase;
mod session;
mod shared;
#[cfg(test)]
mod stubs;
mod worker;

use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    AudioSource, CantodeError, Metadata, PlayerState,
    context::{PlayerContext, PlayerHandle},
    events::{EventSink, PlayerEvent},
    output::AudioSinkFactory,
};

use self::command::{COMMAND_CHANNEL_CAP, LoadResult, SeekResult};
use self::phase::Machine;
use self::shared::SharedStatus;
use self::worker::Worker;

pub(crate) use self::command::Command;

/// The event sinks one player emits to: the context's global sink (if
/// any) plus the per-player sink (if any). Both the [`Machine`] (state
/// events) and the [`Worker`] (operational events) hold a clone.
#[derive(Clone, Default)]
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
        // Resolve the sink factory once, on the handle side: custom if
        // configured, otherwise the default cpal device sink. The worker
        // calls it at the top of every `load`.
        let sink_factory: AudioSinkFactory = config
            .audio_sink_factory
            .unwrap_or_else(|| Arc::new(|| Ok(Box::new(crate::output::CpalSink::new()))));

        let worker_shared = Arc::clone(&shared);
        let sinks = EventSinks {
            cx: cx.event_sink().cloned(),
            player: config.event_sink.clone(),
        };

        let name = cx.next_worker_name();
        let join = thread::Builder::new()
            .name(name)
            .spawn(move || {
                let mut worker = Worker::new(
                    Machine::new(Arc::clone(&worker_shared), sinks.clone()),
                    decoder_factory,
                    cmd_rx,
                    sink_factory,
                    worker_shared,
                    sinks,
                );
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
        self.shared.state()
    }

    /// Current playback position — the media time of the audio currently
    /// being output when the sink tracks it (see
    /// [`AudioSink::output_position`](crate::AudioSink)), otherwise the
    /// decode frontier (which leads the audio by the sink's buffer).
    /// Lock-free read; updated by the worker roughly every 100 ms while
    /// playing (`POSITION_EMIT_INTERVAL` in the worker module).
    pub fn position(&self) -> Duration {
        self.shared.position()
    }

    /// Total duration of the loaded source, if known. `None` before the
    /// first `load` or if the container doesn't report it.
    pub fn duration(&self) -> Option<Duration> {
        self.shared.duration()
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
