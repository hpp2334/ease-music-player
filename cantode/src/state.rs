//! The player state machine.
//!
//! [`PlayerState`] is the public face of the player's progress through the
//! playback lifecycle; [`transition`] is the pure function the worker uses
//! to decide the next state from a current state + an internal worker
//! event. Both are exhaustively tested in `tests/state_machine.rs`.
//!
//! Design notes:
//!
//! - `transition` has **no side effects** and performs **no I/O**. The
//!   worker calls it, then acts on the result. This separation lets us
//!   unit-test the state machine in isolation.
//! - Worker events (`WorkerEvent`) are intentionally distinct from public
//!   [`PlayerEvent`](crate::PlayerEvent): the former are internal causes,
//!   the latter are externally-observable effects.

use crate::CantodeError;

/// Externally-observable player state.
///
/// - `Idle`: no source loaded, no output stream open.
/// - `Loading`: a source is being opened / decoded for the first time.
/// - `Paused`: a source is loaded, the output stream is open but paused.
/// - `Playing`: audio is flowing to the output device.
/// - `Buffering`: the decode pipeline is waiting on the source (network
///   stall, slow disk, ...) — the output stream is still open but will
///   underflow shortly.
/// - `Ended`: the loaded source reached its end. Seeking back resets to
///   `Paused` / `Playing`.
/// - `Error`: an unrecoverable error occurred; the player is wedged until
///   the caller `load`s a fresh source or `stop`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerState {
    /// No source loaded, no output stream open.
    #[default]
    Idle,
    /// A source is being opened / decoded for the first time.
    Loading,
    /// Source loaded, output stream open but paused.
    Paused,
    /// Audio is flowing to the output device.
    Playing,
    /// Decode pipeline waiting on the source (network stall, slow disk).
    Buffering,
    /// Loaded source reached its end. Seeking back resets to `Paused`/`Playing`.
    Ended,
    /// Unrecoverable error; player is wedged until a fresh `load` or `stop`.
    Error,
}

/// Internal worker events — the *causes* of a state transition.
///
/// These are private to the worker; callers never see them. They're
/// `pub(crate)` so the state-machine tests can construct them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerEvent {
    /// `Player::load` was called.
    LoadRequested,
    /// Source opened, decoder ready, sink primed.
    #[allow(dead_code)] // only fired when sink-cpal is enabled
    LoadCompleted,
    /// `Player::play` was called.
    PlayRequested,
    /// `Player::pause` was called.
    PauseRequested,
    /// `Player::stop` was called (drop the loaded source).
    StopRequested,
    /// The source stalled during decode (network / slow I/O). Reserved for
    /// future use by the worker's backpressure detection; currently unused.
    #[allow(dead_code)]
    BufferUnderrun,
    /// The buffer is full again; resume playback. Reserved for future use.
    #[allow(dead_code)]
    BufferRefilled,
    /// The decoder reached end of stream.
    EndOfStream,
    /// An unrecoverable error occurred.
    Failed,
}

/// Compute the next state given a current state and an internal worker
/// event.
///
/// Returns:
/// - `Ok(next)` — the new state. The caller updates the player's state and
///   emits the corresponding [`PlayerEvent::StateChanged`] if it changed.
/// - `Err(CantodeError::InvalidState)` — the event is illegal in this
///   state (e.g. `PlayRequested` while `Idle`). The caller decides whether
///   to surface this to the user.
pub(crate) fn transition(from: PlayerState, ev: WorkerEvent) -> Result<PlayerState, CantodeError> {
    use PlayerState::*;
    use WorkerEvent::*;
    let next = match (from, ev) {
        // ---- Idle ----
        (Idle, LoadRequested) => Loading,
        (Idle, StopRequested) => Idle, // no-op
        // Load/Play/Pause are illegal before any source is loaded.
        (Idle, LoadCompleted | PlayRequested | PauseRequested | BufferUnderrun |
         BufferRefilled | EndOfStream) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Idle"
            )));
        }
        (Idle, Failed) => Error,

        // ---- Loading ----
        (Loading, LoadCompleted) => Paused,
        (Loading, StopRequested) => Idle,
        (Loading, Failed) => Error,
        // User can pre-empt a slow load with play/pause requests; we
        // honour them by staying in Loading (the worker will pick up the
        // desired play-state once LoadCompleted fires).
        (Loading, PlayRequested) => Loading,
        (Loading, PauseRequested) => Loading,
        (Loading, LoadRequested) => Loading,
        (Loading, BufferUnderrun | BufferRefilled | EndOfStream) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Loading"
            )));
        }

        // ---- Paused ----
        (Paused, PlayRequested) => Playing,
        (Paused, PauseRequested) => Paused, // no-op
        (Paused, StopRequested) => Idle,
        (Paused, EndOfStream) => Ended, // a 0-length source
        (Paused, Failed) => Error,
        (Paused, LoadRequested) => Loading,
        (Paused, LoadCompleted | BufferUnderrun | BufferRefilled) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Paused"
            )));
        }

        // ---- Playing ----
        (Playing, PauseRequested) => Paused,
        (Playing, StopRequested) => Idle,
        (Playing, BufferUnderrun) => Buffering,
        (Playing, EndOfStream) => Ended,
        (Playing, Failed) => Error,
        (Playing, LoadRequested) => Loading,
        (Playing, PlayRequested) => Playing, // no-op
        (Playing, LoadCompleted | BufferRefilled) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Playing"
            )));
        }

        // ---- Buffering ----
        (Buffering, BufferRefilled) => Playing,
        (Buffering, PauseRequested) => Paused,
        (Buffering, StopRequested) => Idle,
        (Buffering, EndOfStream) => Ended,
        (Buffering, Failed) => Error,
        (Buffering, LoadRequested) => Loading,
        (Buffering, PlayRequested) => Buffering, // stays; worker picks up on refill
        (Buffering, BufferUnderrun) => Buffering, // no-op
        (Buffering, LoadCompleted) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Buffering"
            )));
        }

        // ---- Ended ----
        (Ended, LoadRequested) => Loading,
        (Ended, PlayRequested | PauseRequested) => Ended, // stuck until load/seek
        (Ended, StopRequested) => Idle,
        (Ended, Failed) => Error,
        (Ended, LoadCompleted | BufferUnderrun | BufferRefilled | EndOfStream) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Ended"
            )));
        }

        // ---- Error ----
        (Error, LoadRequested) => Loading,
        (Error, StopRequested) => Idle,
        (Error, _) => {
            return Err(CantodeError::InvalidState(format!(
                "{ev:?} is illegal in state Error"
            )));
        }
    };
    Ok(next)
}

#[cfg(test)]
mod tests {
    //! Table-driven exhaustive tests for `transition`.
    //!
    //! Each (state, event) pair is enumerated: legal transitions assert
    //! the expected next state; illegal ones assert an error return. The
    //! expected next-states were hand-derived from the state diagram in
    //! the module docs.

    use super::*;

    /// Convenience: shorthand legal-transition assertion.
    fn legal(from: PlayerState, ev: WorkerEvent, want: PlayerState) {
        let got = transition(from, ev).expect("expected legal transition");
        assert_eq!(got, want, "{from:?} + {ev:?} → {got:?} (wanted {want:?})");
    }

    /// Convenience: illegal-transition assertion.
    fn illegal(from: PlayerState, ev: WorkerEvent) {
        let got = transition(from, ev);
        assert!(
            got.is_err(),
            "{from:?} + {ev:?} was allowed ({got:?}); should be illegal"
        );
    }

    #[test]
    fn idle_legal() {
        legal(PlayerState::Idle, WorkerEvent::LoadRequested, PlayerState::Loading);
        legal(PlayerState::Idle, WorkerEvent::StopRequested, PlayerState::Idle);
        legal(PlayerState::Idle, WorkerEvent::Failed, PlayerState::Error);
    }

    #[test]
    fn idle_illegal() {
        for ev in [
            WorkerEvent::LoadCompleted,
            WorkerEvent::PlayRequested,
            WorkerEvent::PauseRequested,
            WorkerEvent::BufferUnderrun,
            WorkerEvent::BufferRefilled,
            WorkerEvent::EndOfStream,
        ] {
            illegal(PlayerState::Idle, ev);
        }
    }

    #[test]
    fn loading_legal() {
        legal(PlayerState::Loading, WorkerEvent::LoadCompleted, PlayerState::Paused);
        legal(PlayerState::Loading, WorkerEvent::StopRequested, PlayerState::Idle);
        legal(PlayerState::Loading, WorkerEvent::Failed, PlayerState::Error);
        // Pre-emptive play/pause during load: stay in Loading.
        legal(PlayerState::Loading, WorkerEvent::PlayRequested, PlayerState::Loading);
        legal(PlayerState::Loading, WorkerEvent::PauseRequested, PlayerState::Loading);
        legal(PlayerState::Loading, WorkerEvent::LoadRequested, PlayerState::Loading);
    }

    #[test]
    fn loading_illegal() {
        for ev in [
            WorkerEvent::BufferUnderrun,
            WorkerEvent::BufferRefilled,
            WorkerEvent::EndOfStream,
        ] {
            illegal(PlayerState::Loading, ev);
        }
    }

    #[test]
    fn paused_legal() {
        legal(PlayerState::Paused, WorkerEvent::PlayRequested, PlayerState::Playing);
        legal(PlayerState::Paused, WorkerEvent::PauseRequested, PlayerState::Paused);
        legal(PlayerState::Paused, WorkerEvent::StopRequested, PlayerState::Idle);
        legal(PlayerState::Paused, WorkerEvent::EndOfStream, PlayerState::Ended);
        legal(PlayerState::Paused, WorkerEvent::Failed, PlayerState::Error);
        legal(PlayerState::Paused, WorkerEvent::LoadRequested, PlayerState::Loading);
    }

    #[test]
    fn playing_legal() {
        legal(PlayerState::Playing, WorkerEvent::PauseRequested, PlayerState::Paused);
        legal(PlayerState::Playing, WorkerEvent::StopRequested, PlayerState::Idle);
        legal(PlayerState::Playing, WorkerEvent::BufferUnderrun, PlayerState::Buffering);
        legal(PlayerState::Playing, WorkerEvent::EndOfStream, PlayerState::Ended);
        legal(PlayerState::Playing, WorkerEvent::Failed, PlayerState::Error);
        legal(PlayerState::Playing, WorkerEvent::LoadRequested, PlayerState::Loading);
        legal(PlayerState::Playing, WorkerEvent::PlayRequested, PlayerState::Playing);
    }

    #[test]
    fn buffering_legal() {
        legal(PlayerState::Buffering, WorkerEvent::BufferRefilled, PlayerState::Playing);
        legal(PlayerState::Buffering, WorkerEvent::PauseRequested, PlayerState::Paused);
        legal(PlayerState::Buffering, WorkerEvent::StopRequested, PlayerState::Idle);
        legal(PlayerState::Buffering, WorkerEvent::EndOfStream, PlayerState::Ended);
        legal(PlayerState::Buffering, WorkerEvent::Failed, PlayerState::Error);
        legal(PlayerState::Buffering, WorkerEvent::LoadRequested, PlayerState::Loading);
        legal(PlayerState::Buffering, WorkerEvent::BufferUnderrun, PlayerState::Buffering);
    }

    #[test]
    fn ended_legal() {
        legal(PlayerState::Ended, WorkerEvent::LoadRequested, PlayerState::Loading);
        legal(PlayerState::Ended, WorkerEvent::StopRequested, PlayerState::Idle);
        legal(PlayerState::Ended, WorkerEvent::Failed, PlayerState::Error);
        legal(PlayerState::Ended, WorkerEvent::PlayRequested, PlayerState::Ended);
        legal(PlayerState::Ended, WorkerEvent::PauseRequested, PlayerState::Ended);
    }

    #[test]
    fn error_legal() {
        legal(PlayerState::Error, WorkerEvent::LoadRequested, PlayerState::Loading);
        legal(PlayerState::Error, WorkerEvent::StopRequested, PlayerState::Idle);
    }

    #[test]
    fn error_illegal() {
        for ev in [
            WorkerEvent::LoadCompleted,
            WorkerEvent::PlayRequested,
            WorkerEvent::PauseRequested,
            WorkerEvent::BufferUnderrun,
            WorkerEvent::BufferRefilled,
            WorkerEvent::EndOfStream,
            WorkerEvent::Failed,
        ] {
            illegal(PlayerState::Error, ev);
        }
    }
}
