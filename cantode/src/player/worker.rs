//! The worker: event loop and the operations it dispatches to.
//!
//! The worker (not the public API) is the sole owner of the decoder and
//! the sink. This keeps the cpal `Stream` on a single thread for its
//! whole lifetime — the discipline cpal/AAudio/CoreAudio require for
//! real-time audio.
//!
//! The loop parks in `recv_timeout`: **5 ms while `Playing`** (so decode
//! work interleaves with command handling — a command arriving mid-wait
//! wakes the loop instantly; the timeout is the idle fallback, not added
//! latency), and one hour otherwise (pure event-driven idling, with the
//! timeout acting only as a watchdog). Each timeout tick pumps exactly
//! one frame. The pump loop is a buffer-*filler*, not the pacer: the
//! sink's blocking write matches decode speed to playback speed once its
//! ring is full, and the 5 ms quantum just guarantees the worker
//! re-enters `recv_timeout` between every frame so queued commands
//! preempt within one pump.

use std::{
    mem,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AudioSource, CantodeError, Metadata, PlayerState,
    decoder::DecoderFactory,
    events::{EventSink, PlayerEvent},
    output::AudioSinkFactory,
    state::{WorkerEvent, transition},
};

use super::command::{Command, LoadResult, SeekResult};
use super::phase::{Phase, morph};
use super::session::Loaded;
use super::shared::SharedStatus;

/// How often the worker emits [`PlayerEvent::PositionChanged`] while
/// playing. 10 Hz matches typical UI polling cadences and keeps the
/// event channel from saturating.
const POSITION_EMIT_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct Worker {
    /// Worker-private state machine state; the single source of truth for
    /// what the player is doing and what it owns. Projected into
    /// `shared.state` by `set_phase` — the mirror's only writer.
    phase: Phase,
    decoder_factory: Arc<dyn DecoderFactory>,
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

pub(super) struct EventSinks {
    pub(super) cx: Option<Arc<dyn EventSink>>,
    pub(super) player: Option<Arc<dyn EventSink>>,
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
    /// Assemble a worker. Called by `Player::with_config` inside the
    /// spawn closure; the fields stay private to this module.
    pub(super) fn new(
        phase: Phase,
        decoder_factory: Arc<dyn DecoderFactory>,
        cmd_rx: mpsc::Receiver<Command>,
        sink_factory: AudioSinkFactory,
        shared: Arc<SharedStatus>,
        sinks: EventSinks,
    ) -> Self {
        Self {
            phase,
            decoder_factory,
            cmd_rx,
            sink_factory,
            shared,
            sinks,
        }
    }

    pub(super) fn run(&mut self) {
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
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if matches!(self.phase, Phase::Playing { .. }) {
                        self.pump_once();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
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

#[cfg(test)]
mod tests {
    //! Unit tests for the worker's operations (`do_seek`, `do_stop`)
    //! against stub phases, using the doubles from
    //! `crate::player::stubs`. No audio device needed.

    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::CantodeError;
    use crate::player::stubs::{StubFactory, loaded_session};

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
