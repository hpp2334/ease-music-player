//! The worker: event loop and the operations it dispatches to.
//!
//! The worker (not the public API) is the sole owner of the decoder and
//! the sink. This keeps the cpal `Stream` on a single thread for its
//! whole lifetime — the discipline cpal/AAudio/CoreAudio require for
//! real-time audio.
//!
//! State transitions are *requested* from the
//! [`Machine`](super::phase::Machine) — the worker names intents
//! (`play`, `pause`, `fail`, …), never causes or phases. The worker's
//! own writes are the session-independent observables (`position`,
//! `duration`) and the operational events (`MetadataReady`,
//! `PositionChanged`, `Ended`).
//!
//! The loop parks in `recv_timeout`: **5 ms while `Playing` or
//! `Buffering`** (so decode work / refill polls interleave with command
//! handling — a command arriving mid-wait wakes the loop instantly; the
//! timeout is the idle fallback, not added latency), and one hour
//! otherwise (pure event-driven idling, with the timeout acting only as
//! a watchdog). Each timeout tick pumps exactly one frame while playing,
//! or polls the source's readiness while buffering. The pump loop is a
//! buffer-*filler*, not the pacer: the sink's blocking write matches
//! decode speed to playback speed once its ring is full, and the 5 ms
//! quantum just guarantees the worker re-enters `recv_timeout` between
//! every frame so queued commands preempt within one pump. A starved
//! source surfaces as `Playing → Buffering` (readiness pre-check, or the
//! 250 ms play-path read deadline) and back on refill — the sink keeps
//! draining its ring across the morph.

use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

use crate::{
    AudioSource, CantodeError, Metadata, decoder::DecoderFactory, events::PlayerEvent,
    output::AudioSinkFactory,
};

use super::EventSinks;
use super::command::{Command, LoadResult, SeekResult};
use super::phase::Machine;
use super::session::{Loaded, PumpOutcome};
use super::shared::SharedStatus;

/// How often the worker emits [`PlayerEvent::PositionChanged`] while
/// playing. 10 Hz matches typical UI polling cadences and keeps the
/// event channel from saturating. Passed into `Loaded::pump` — event
/// cadence is the worker's policy, not the session's.
const POSITION_EMIT_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct Worker {
    /// The state-machine core: owns the phase and the shared state
    /// mirror. The worker only requests transitions from it.
    machine: Machine,
    decoder_factory: Arc<dyn DecoderFactory>,
    cmd_rx: mpsc::Receiver<Command>,
    /// Constructs the sink for each loaded source (custom, or the default
    /// cpal device sink when the config didn't provide one).
    sink_factory: AudioSinkFactory,
    /// Observable projection shared with the `Player` handle. The worker
    /// only publishes `duration` here (after a successful load);
    /// `state` belongs to the machine, `position` to the session.
    shared: Arc<SharedStatus>,
    /// Operational events (`MetadataReady`, `PositionChanged`, `Ended`).
    /// `StateChanged` / illegal-transition errors come from the machine's
    /// own copy.
    sinks: EventSinks,
    /// Dedup latch for source-error events: a failing source errors on
    /// every pump; the UI wants one `PlayerEvent::Error` per episode,
    /// not a stream of them. Cleared by a successful seek (the classic
    /// user-driven recovery).
    error_latched: bool,
}

impl Worker {
    /// Assemble a worker. Called by `Player::with_config` inside the
    /// spawn closure; the fields stay private to this module.
    pub(super) fn new(
        machine: Machine,
        decoder_factory: Arc<dyn DecoderFactory>,
        cmd_rx: mpsc::Receiver<Command>,
        sink_factory: AudioSinkFactory,
        shared: Arc<SharedStatus>,
        sinks: EventSinks,
    ) -> Self {
        Self {
            machine,
            decoder_factory,
            cmd_rx,
            sink_factory,
            shared,
            sinks,
            error_latched: false,
        }
    }

    pub(super) fn run(&mut self) {
        loop {
            // If we're playing (or buffering, waiting on the source), use
            // a short timeout so decode work / refill polls interleave
            // with command handling; otherwise block on commands.
            let timeout = if self.machine.is_playing() || self.machine.is_buffering() {
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
                    if self.machine.is_playing() {
                        self.pump_once();
                    } else if self.machine.is_buffering() {
                        self.poll_refill();
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
            Command::Play => self.machine.play(),
            Command::Pause => self.machine.pause(),
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
                if let Some(loaded) = self.machine.loaded_mut() {
                    loaded.set_volume(v);
                }
            }
        }
        false
    }

    fn do_load(&mut self, source: Box<dyn AudioSource>) -> crate::Result<Metadata> {
        // Discard any existing session (old sink stops, session-scoped
        // observables reset) and publish `Loading` — in that order, per
        // `Machine::begin_load`.
        self.machine.begin_load();

        let opened = self.decoder_factory.open(source);
        let dec = match opened {
            Ok(dec) => dec,
            Err(e) => {
                self.machine.fail();
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
                self.machine.fail();
                return Err(e);
            }
        };
        let actual_fmt = match sink.start(fmt) {
            Ok(actual_fmt) => actual_fmt,
            Err(e) => {
                self.machine.fail();
                return Err(e);
            }
        };

        let loaded = Loaded::new(dec, sink, actual_fmt, Arc::clone(&self.shared));

        // Publish metadata/duration only once the sink is up (the success
        // path): a failed load leaves the observables cleared by the old
        // session's drop, rather than half-updated.
        self.shared.set_duration(meta.duration);
        self.sinks.emit(PlayerEvent::MetadataReady(meta.clone()));

        // Commit the fresh session as `Paused` (validated against the
        // transition table inside the machine).
        self.machine.complete_load(loaded);
        Ok(meta)
    }

    fn do_stop(&mut self) {
        // `StopRequested → Idle`. The machine drops the session (if any),
        // which stops the sink and resets position/duration. From `Idle`
        // the transition is a no-op and nothing runs.
        self.machine.stop();
    }

    fn do_unload(&mut self) -> crate::Result<()> {
        self.do_stop();
        Ok(())
    }

    fn do_seek(&mut self, target: Duration) -> crate::Result<Duration> {
        let Some(loaded) = self.machine.loaded_mut() else {
            return Err(CantodeError::InvalidState(
                "seek requires a loaded source".into(),
            ));
        };
        // The session performs the choreography (decoder seek + sink
        // flush + latch clear + position publish); we announce it.
        let actual = loaded.seek(target)?;
        self.sinks.emit(PlayerEvent::PositionChanged(actual));
        // A successful seek is the classic user-driven recovery — the
        // source-error episode (if any) is over.
        self.error_latched = false;
        Ok(actual)
    }

    /// Decode one frame and push it to the sink. Called from the
    /// Playing-loop body.
    fn pump_once(&mut self) {
        // Stage 1 — the session's decode→render step under the playing
        // borrow. It stores the position observable itself and returns
        // the emission decisions. The readiness pre-check skips the read
        // entirely when the window is starved (a read would park up to
        // the deadline and defer commands behind it).
        let outcome = {
            let Some((loaded, last_position_emit)) = self.machine.playing_mut() else {
                return;
            };
            if loaded.readiness() == crate::Readiness::NeedsData {
                PumpOutcome::NeedsData
            } else {
                loaded.pump(last_position_emit, POSITION_EMIT_INTERVAL)
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
                if let Some(loaded) = self.machine.loaded_mut()
                    && !loaded.has_ended()
                {
                    loaded.mark_ended();
                    self.sinks.emit(PlayerEvent::Ended);
                }
                self.machine.end_of_stream();
            }
            PumpOutcome::NeedsData => {
                // Starved but alive: park the pump (the sink drains its
                // ring) until the refill poll sees data again.
                self.machine.buffer_underrun();
            }
            PumpOutcome::Skipped(err) => {
                if let Some(e) = err {
                    self.report_source_error(e);
                }
            }
        }
    }

    /// While `Buffering`: poll the source's readiness and morph back to
    /// `Playing` once data has arrived.
    fn poll_refill(&mut self) {
        if let Some(loaded) = self.machine.loaded_mut()
            && loaded.readiness() == crate::Readiness::Ready
        {
            self.machine.buffer_refilled();
        }
    }

    /// Surface a source/decode error as `PlayerEvent::Error` — once per
    /// episode (a failing source errors on every pump; the UI doesn't
    /// want a stream of identical events). Cleared by a successful seek.
    fn report_source_error(&mut self, e: CantodeError) {
        if self.error_latched {
            return;
        }
        self.error_latched = true;
        self.sinks.emit(PlayerEvent::Error(e));
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the worker's operations (`do_seek`, `do_stop`)
    //! against a stub-backed machine, using the doubles from
    //! `crate::player::stubs`. No audio device needed.

    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::CantodeError;
    use crate::player::stubs::{StubFactory, loaded_session};
    use crate::state::PlayerState;

    fn worker_with(machine: Machine, shared: Arc<SharedStatus>) -> Worker {
        let (_tx, rx) = mpsc::channel();
        Worker {
            machine,
            decoder_factory: Arc::new(StubFactory),
            cmd_rx: rx,
            sink_factory: Arc::new(|| Err(CantodeError::Internal("stub factory".into()))),
            shared,
            sinks: EventSinks::default(),
            error_latched: false,
        }
    }

    #[test]
    fn seek_requires_a_session() {
        let shared = Arc::new(SharedStatus::new());
        let mut worker = worker_with(
            Machine::new(Arc::clone(&shared), EventSinks::default()),
            shared,
        );
        let err = worker.do_seek(Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, CantodeError::InvalidState(_)));
    }

    #[test]
    fn seek_flushes_the_sink_and_clears_the_ended_latch() {
        let (loaded, fx) = loaded_session(2, 2);
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );

        // Simulate an already-ended session, then seek back into it.
        worker.machine.loaded_mut().unwrap().mark_ended();
        let actual = worker.do_seek(Duration::from_secs(5)).unwrap();

        assert_eq!(actual, Duration::from_secs(5));
        assert!(fx.log.recorded("flush"));
        assert!(!worker.machine.loaded_mut().unwrap().has_ended());
        assert_eq!(fx.shared.position(), Duration::from_secs(5));
    }

    #[test]
    fn stop_tears_the_session_down_and_resets_observables() {
        let (loaded, fx) = loaded_session(2, 2);
        let mut worker = worker_with(
            Machine::paused(loaded, Arc::clone(&fx.shared)),
            Arc::clone(&fx.shared),
        );
        fx.shared.set_position(Duration::from_secs(9));
        fx.shared.set_duration(Some(Duration::from_secs(9)));

        worker.do_stop();

        assert_eq!(worker.machine.state(), PlayerState::Idle);
        assert_eq!(fx.shared.state(), PlayerState::Idle);
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position(), Duration::ZERO);
        assert_eq!(fx.shared.duration(), None);
    }
}
