//! The observable projection of the worker's `Phase`.
//!
//! The worker is the sole writer; the `Player` handle reads these
//! lock-free from other threads. This is deliberately *not* part of
//! `Phase`: `Phase` owns `!Sync` resources (decoder, sink) that must
//! never leave the worker thread, while these status registers must stay
//! readable at any moment — including while the worker is mid-`load` or
//! blocked in a backpressured sink write, and in the payload-less phases
//! (`Idle` / `Loading` / `Error`) where there is no session to hang data
//! on.
//!
//! Fields are private and access is method-only, which is also how the
//! ownership split is enforced: `set_state` is called by the
//! [`Machine`](super::phase::Machine) alone, `set_position` by the
//! [`Loaded`](super::session::Loaded) session alone, `set_duration` by
//! the worker's load path alone, and
//! `reset_session_observables` by `Loaded::drop` alone.

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use crate::state::PlayerState;

pub(super) struct SharedStatus {
    /// Mirrored by the machine's commit — its only writer.
    state: AtomicState,
    /// Last decoded-frame timestamp; reset by `Loaded::drop`.
    position: AtomicPosition,
    /// Duration of the loaded source; set by the worker on load success,
    /// reset by `Loaded::drop`.
    duration: Mutex<Option<Duration>>,
}

impl SharedStatus {
    pub(super) fn new() -> Self {
        Self {
            state: AtomicState::new(PlayerState::Idle),
            position: AtomicPosition::new(),
            duration: Mutex::new(None),
        }
    }

    /// The externally-observable state. Lock-free.
    pub(super) fn state(&self) -> PlayerState {
        self.state.load()
    }

    /// Mirror a committed phase. [`Machine::commit`] only.
    pub(super) fn set_state(&self, s: PlayerState) {
        self.state.store(s);
    }

    /// Last decoded-frame timestamp. Lock-free.
    pub(super) fn position(&self) -> Duration {
        self.position.load()
    }

    /// Publish the position (decode / seek). The [`Loaded`] session only.
    pub(super) fn set_position(&self, d: Duration) {
        self.position.store(d);
    }

    /// Duration of the loaded source, if known.
    pub(super) fn duration(&self) -> Option<Duration> {
        *self.duration.lock().unwrap()
    }

    /// Publish the duration after a successful load. Worker only.
    pub(super) fn set_duration(&self, d: Option<Duration>) {
        *self.duration.lock().unwrap() = d;
    }

    /// Reset the session-scoped observables (position, duration).
    /// `Loaded::drop` only — session death is the one reset point.
    pub(super) fn reset_session_observables(&self) {
        self.position.store(Duration::ZERO);
        *self.duration.lock().unwrap() = None;
    }
}

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
