//! The worker-side state machine shapes.
//!
//! `Phase` is the data-carrying twin of the public
//! [`PlayerState`](crate::state::PlayerState): every variant with a live
//! decode session carries its `Loaded` payload, so the invariants
//! "sink open ⟹ decoder open" and "conversion state exists ⟺ session
//! exists" hold by construction.
//!
//! `Phase` is worker-private (its payloads are `!Sync`); the public state
//! is the projection `Phase::state`, mirrored into `SharedStatus` by the
//! worker. Transitions are still decided by the pure
//! [`transition`](crate::state::transition) function over the projected
//! state; `morph` then reshapes the payload to match.

use std::fmt;
use std::time::Instant;

use crate::state::PlayerState;

use super::session::Loaded;

/// Worker-side state machine state: the data-carrying twin of the public
/// [`PlayerState`]. Every variant with a live decode session carries its
/// [`Loaded`] payload, so the invariants "sink open ⟹ decoder open" and
/// "conversion state exists ⟺ session exists" hold by construction.
pub(super) enum Phase {
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
    pub(super) fn state(&self) -> PlayerState {
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
    pub(super) fn loaded_mut(&mut self) -> Option<&mut Loaded> {
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
/// drift. `Paused` is reached only by `do_load`'s inline completion
/// (from `Loading`, which carries no session), never through `morph`.
pub(super) fn morph(current: Phase, next: PlayerState) -> Phase {
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

#[cfg(test)]
mod tests {
    //! Unit tests for the phase machinery — projection and `morph` —
    //! using the stub doubles from `crate::player::stubs`.

    use std::time::Duration;

    use super::*;
    use crate::player::stubs::loaded_session;

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
}
