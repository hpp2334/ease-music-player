//! The worker-side state machine: `Phase` shapes and the `Machine` that
//! owns them.
//!
//! `Phase` is the data-carrying twin of the public
//! [`PlayerState`](crate::state::PlayerState): every variant with a live
//! decode session carries its `Loaded` payload, so the invariants
//! "sink open ⟹ decoder open" and "conversion state exists ⟺ session
//! exists" hold by construction.
//!
//! [`Machine`] is the **sole owner** of the `Phase` and of the shared
//! state mirror. The worker can only *request* transitions — through
//! `apply` (the one gate for cause-driven transitions), or through the
//! two named lifecycle composites `begin_load` / `complete_load` (the
//! only moves that carry a fresh session). Nothing outside this file can
//! name a `Phase` variant, reshape a session, or write the mirror;
//! `Phase` and `morph` are file-private on purpose.
//!
//! Transitions are still decided by the pure
//! [`transition`](crate::state::transition) function over the projected
//! state; `morph` then reshapes the payload to match.

use std::{fmt, mem, sync::Arc, time::Instant};

use crate::events::PlayerEvent;
use crate::state::{PlayerState, WorkerEvent, transition};

use super::EventSinks;
use super::session::Loaded;
use super::shared::SharedStatus;

/// Worker-side state machine state: the data-carrying twin of the public
/// [`PlayerState`]. Every variant with a live decode session carries its
/// [`Loaded`] payload, so the invariants "sink open ⟹ decoder open" and
/// "conversion state exists ⟺ session exists" hold by construction.
///
/// Private to this module: only [`Machine`] may touch a `Phase`.
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
/// [`transition`](crate::state::transition) table or the call sites
/// drift. `Paused` is reached only by `complete_load` (from `Loading`,
/// which carries no session), never through `morph`.
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

/// The state-machine core: sole owner of the worker's `Phase` and the
/// single writer of the shared state mirror. The worker requests
/// transitions; only this type can commit them, so the mirror can never
/// disagree with the phase and no transition can bypass the
/// [`transition`](crate::state::transition) table.
///
/// (The phase payloads are `!Sync`, which is also why this lives on the
/// worker thread and only its projections cross threads.)
pub(super) struct Machine {
    phase: Phase,
    /// Observable projection shared with the `Player` handle; written
    /// only by [`Machine::commit`].
    shared: Arc<SharedStatus>,
    /// Emits `StateChanged` (and illegal-transition `Error`) events.
    sinks: EventSinks,
}

impl Machine {
    /// A machine starting at `Idle`.
    pub(super) fn new(shared: Arc<SharedStatus>, sinks: EventSinks) -> Self {
        Self {
            phase: Phase::Idle,
            shared,
            sinks,
        }
    }

    /// The externally-observable state.
    #[cfg(test)]
    pub(super) fn state(&self) -> PlayerState {
        self.phase.state()
    }

    /// Whether the decode loop should be pumping.
    pub(super) fn is_playing(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. })
    }

    /// Whether the session is parked waiting on source data (the worker
    /// keeps its short tick to poll for the refill).
    pub(super) fn is_buffering(&self) -> bool {
        matches!(self.phase, Phase::Buffering(_))
    }

    /// The live session, if any. Session access is not a transition —
    /// the worker uses this to seek, set volume, and latch `Ended`.
    pub(super) fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        self.phase.loaded_mut()
    }

    /// The live session plus the position-event throttle timestamp, when
    /// playing. The worker's decode step runs under this borrow.
    pub(super) fn playing_mut(&mut self) -> Option<(&mut Loaded, &mut Instant)> {
        match &mut self.phase {
            Phase::Playing {
                loaded,
                last_position_emit,
            } => Some((loaded, last_position_emit)),
            _ => None,
        }
    }

    /// The one gate for cause-driven transitions: look up
    /// [`transition`](crate::state::transition), reshape the payload via
    /// `morph`, and commit. Illegal transitions surface as a non-fatal
    /// `PlayerEvent::Error` instead of wedging the machine.
    ///
    /// Private to this module: callers use the intent methods below.
    fn apply(&mut self, ev: WorkerEvent) {
        let from = self.phase.state();
        match transition(from, ev) {
            Ok(next) => {
                if next != from {
                    // Take the current phase out, reshape it to the new
                    // state's shape (moving or dropping the session), and
                    // commit.
                    let current = mem::replace(&mut self.phase, Phase::Idle);
                    self.commit(morph(current, next));
                }
            }
            Err(e) => {
                // Illegal transition — surface as a non-fatal error event.
                self.sinks.emit(PlayerEvent::Error(e));
            }
        }
    }

    // ----- intent methods: the worker's vocabulary for transitions -----

    /// Resume / start playback (`PlayRequested`).
    pub(super) fn play(&mut self) {
        self.apply(WorkerEvent::PlayRequested);
    }

    /// Suspend playback (`PauseRequested`).
    pub(super) fn pause(&mut self) {
        self.apply(WorkerEvent::PauseRequested);
    }

    /// Drop the loaded session and return to `Idle` (`StopRequested`).
    pub(super) fn stop(&mut self) {
        self.apply(WorkerEvent::StopRequested);
    }

    /// An unrecoverable error occurred (`Failed`).
    pub(super) fn fail(&mut self) {
        self.apply(WorkerEvent::Failed);
    }

    /// The decoder reached end of stream (`EndOfStream`).
    pub(super) fn end_of_stream(&mut self) {
        self.apply(WorkerEvent::EndOfStream);
    }

    /// The source starved mid-play (`BufferUnderrun`): morph
    /// `Playing → Buffering` — the sink keeps draining its ring while the
    /// worker polls for the refill.
    pub(super) fn buffer_underrun(&mut self) {
        self.apply(WorkerEvent::BufferUnderrun);
    }

    /// Data arrived again (`BufferRefilled`): morph `Buffering → Playing`
    /// and resume pumping.
    pub(super) fn buffer_refilled(&mut self) {
        self.apply(WorkerEvent::BufferRefilled);
    }

    /// Start a load: discard any existing session, then publish
    /// `Loading`.
    ///
    /// The discard is deliberately **not** a commit — the old sink stops
    /// and the session-scoped observables reset via `Loaded::drop`, but
    /// the shared mirror keeps reporting the old state until the
    /// `LoadRequested` transition below publishes `Loading`. (A polling
    /// `Player::state()` reader sees `Paused → Loading`, never a
    /// phantom `Idle`.) This ordering is load-bearing; it lives here so
    /// no caller can get it wrong.
    pub(super) fn begin_load(&mut self) {
        self.phase = Phase::Idle;
        self.apply(WorkerEvent::LoadRequested);
    }

    /// Finish a load by committing the fresh session as `Paused`.
    ///
    /// The only transition that carries a *fresh* session, which is why
    /// it bypasses `morph` (`Loading` has no payload to reshape). Still
    /// validated against the [`transition`](crate::state::transition)
    /// table so it stays the single authority on what a completed load
    /// means. Runs synchronously on the worker thread, so no command can
    /// interleave and no pending-play bookkeeping is needed.
    pub(super) fn complete_load(&mut self, loaded: Loaded) {
        debug_assert!(matches!(self.phase, Phase::Loading));
        let next = transition(PlayerState::Loading, WorkerEvent::LoadCompleted)
            .expect("LoadCompleted is legal from Loading");
        self.commit(Phase::Paused(loaded));
        debug_assert_eq!(next, PlayerState::Paused);
    }

    /// Commit a new phase: install it, mirror it, announce it, run its
    /// side effects. The single writer of the shared state mirror.
    fn commit(&mut self, phase: Phase) {
        let state = phase.state();
        self.phase = phase;
        self.shared.set_state(state);
        self.sinks.emit(PlayerEvent::StateChanged(state));
        self.on_phase_changed();
    }

    /// Side effects to run after a phase change.
    fn on_phase_changed(&mut self) {
        match &mut self.phase {
            Phase::Playing { loaded, .. } => {
                loaded.resume();
            }
            Phase::Paused(loaded) | Phase::Ended(loaded) => {
                // Pause the sink so the device stops outputting whatever
                // samples it had buffered (on `Ended`, the tail of the
                // source).
                loaded.pause();
            }
            // `Buffering` keeps the stream running on purpose: the sink
            // ring buffer drains what it has while the source stalls.
            Phase::Buffering(_) | Phase::Idle | Phase::Loading | Phase::Error => {}
        }
    }

    /// Test constructor: a machine resting in `Paused` on the given
    /// session, mirroring into the given shared status (they are one
    /// unit in production too).
    #[cfg(test)]
    pub(super) fn paused(loaded: Loaded, shared: Arc<SharedStatus>) -> Self {
        Self {
            phase: Phase::Paused(loaded),
            shared,
            sinks: EventSinks::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the phase machinery — projection, `morph`, and the
    //! `Machine` gate — using the stub doubles from `crate::player::stubs`.

    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::CantodeError;
    use crate::events::EventSink;
    use crate::player::stubs::loaded_session;

    /// Event sink that records everything it receives.
    #[derive(Default)]
    struct RecordingSink {
        events: Mutex<Vec<PlayerEvent>>,
    }

    impl EventSink for RecordingSink {
        fn emit(&self, event: PlayerEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn recorded_states(rec: &RecordingSink) -> Vec<PlayerState> {
        rec.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|ev| match ev {
                PlayerEvent::StateChanged(s) => Some(*s),
                _ => None,
            })
            .collect()
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
        // log is shared — a `pause` through the moved session shows up in
        // it), and it was never stopped or reset.
        let session = phase.loaded_mut().expect("Ended carries the session");
        session.pause();
        assert!(fx.log.recorded("pause"));
        assert!(!fx.log.recorded("stop"));
    }

    #[test]
    fn morph_moves_do_not_reset_observables() {
        // Position/duration reset only on session *exit*, never on a
        // variant-to-variant move (which destructures, not drops).
        let (loaded, fx) = loaded_session(2, 2);
        fx.shared.set_position(Duration::from_secs(42));
        fx.shared.set_duration(Some(Duration::from_secs(60)));

        let phase = morph(Phase::Paused(loaded), PlayerState::Playing);
        let phase = morph(phase, PlayerState::Buffering);
        let _phase = morph(phase, PlayerState::Ended);

        assert_eq!(fx.shared.position(), Duration::from_secs(42));
        assert_eq!(fx.shared.duration(), Some(Duration::from_secs(60)));
    }

    #[test]
    fn morph_drops_the_session_on_unloaded_targets() {
        for target in [PlayerState::Idle, PlayerState::Loading, PlayerState::Error] {
            let (loaded, fx) = loaded_session(2, 2);
            fx.shared.set_position(Duration::from_secs(7));
            fx.shared.set_duration(Some(Duration::from_secs(7)));

            let phase = morph(Phase::Paused(loaded), target);
            assert_eq!(phase.state(), target);

            // The drop ran: sink stopped, observables reset.
            assert!(fx.log.recorded("stop"), "sink not stopped for {target:?}");
            assert_eq!(fx.shared.position(), Duration::ZERO);
            assert_eq!(fx.shared.duration(), None);
        }
    }

    #[test]
    fn begin_load_discards_the_session_and_publishes_loading() {
        let (loaded, fx) = loaded_session(2, 2);
        let mut machine = Machine::paused(loaded, Arc::clone(&fx.shared));
        // Put the mirror where the phase is, as production would have it.
        fx.shared.set_state(PlayerState::Paused);

        machine.begin_load();

        // The old session tore down exactly once (stop + observable
        // resets)…
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position(), Duration::ZERO);
        assert_eq!(fx.shared.duration(), None);
        // …and the machine published `Loading`, not a phantom `Idle`.
        assert_eq!(machine.state(), PlayerState::Loading);
        assert_eq!(fx.shared.state(), PlayerState::Loading);
    }

    #[test]
    fn complete_load_publishes_paused_and_pauses_the_sink() {
        let (loaded, fx) = loaded_session(2, 2);
        let shared = Arc::clone(&fx.shared);
        let rec = Arc::new(RecordingSink::default());
        let mut machine = Machine::new(
            shared,
            EventSinks {
                cx: None,
                player: Some(rec.clone()),
            },
        );

        machine.begin_load();
        machine.complete_load(loaded);

        assert_eq!(machine.state(), PlayerState::Paused);
        assert_eq!(fx.shared.state(), PlayerState::Paused);
        // Phase-commit side effect: a paused sink stops outputting.
        assert!(fx.log.recorded("pause"));
        // The fresh session is alive (not stopped) and was announced.
        assert!(!fx.log.recorded("stop"));
        assert_eq!(
            recorded_states(&rec),
            vec![PlayerState::Loading, PlayerState::Paused]
        );
    }

    #[test]
    fn apply_emits_error_on_illegal_event_instead_of_wedging() {
        let rec = Arc::new(RecordingSink::default());
        let mut machine = Machine::new(
            Arc::new(SharedStatus::new()),
            EventSinks {
                cx: None,
                player: Some(rec.clone()),
            },
        );

        // PlayRequested is illegal from Idle — through the public door.
        machine.play();

        assert_eq!(machine.state(), PlayerState::Idle);
        assert!(matches!(
            rec.events.lock().unwrap().first(),
            Some(PlayerEvent::Error(CantodeError::InvalidState(_)))
        ));
    }

    #[test]
    fn underrun_and_refill_move_the_session_between_playing_and_buffering() {
        let (loaded, fx) = loaded_session(2, 2);
        let shared = Arc::clone(&fx.shared);
        let rec = Arc::new(RecordingSink::default());
        let mut machine = Machine::new(
            shared,
            EventSinks {
                cx: None,
                player: Some(rec.clone()),
            },
        );
        machine.begin_load();
        machine.complete_load(loaded);
        machine.play();

        machine.buffer_underrun();
        assert_eq!(machine.state(), PlayerState::Buffering);
        assert_eq!(fx.shared.state(), PlayerState::Buffering);

        machine.buffer_refilled();
        assert_eq!(machine.state(), PlayerState::Playing);
        assert_eq!(fx.shared.state(), PlayerState::Playing);

        // The session survived both morphs: same sink, never stopped.
        assert!(!fx.log.recorded("stop"));
        assert_eq!(
            recorded_states(&rec),
            vec![
                PlayerState::Loading,
                PlayerState::Paused,
                PlayerState::Playing,
                PlayerState::Buffering,
                PlayerState::Playing
            ]
        );
    }

    #[test]
    fn buffer_underrun_is_illegal_outside_playing() {
        let rec = Arc::new(RecordingSink::default());
        let mut machine = Machine::new(
            Arc::new(SharedStatus::new()),
            EventSinks {
                cx: None,
                player: Some(rec.clone()),
            },
        );

        machine.buffer_underrun(); // illegal from Idle

        assert_eq!(machine.state(), PlayerState::Idle);
        assert!(matches!(
            rec.events.lock().unwrap().first(),
            Some(PlayerEvent::Error(_))
        ));
    }
}
