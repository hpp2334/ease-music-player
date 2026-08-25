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

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use crate::state::PlayerState;

pub(super) struct SharedStatus {
    /// Mirrored by the worker's `set_phase` — its only writer.
    pub(super) state: AtomicState,
    /// Last decoded-frame timestamp; reset by `Loaded::drop`.
    pub(super) position: AtomicPosition,
    /// Duration of the loaded source; set by `do_load` on success, reset
    /// by `Loaded::drop`.
    pub(super) duration: Mutex<Option<Duration>>,
}

impl SharedStatus {
    pub(super) fn new() -> Self {
        Self {
            state: AtomicState::new(PlayerState::Idle),
            position: AtomicPosition::new(),
            duration: Mutex::new(None),
        }
    }
}

pub(super) struct AtomicState {
    bits: AtomicU64,
}

impl AtomicState {
    fn new(initial: PlayerState) -> Self {
        Self {
            bits: AtomicU64::new(initial as u64),
        }
    }
    pub(super) fn store(&self, s: PlayerState) {
        self.bits.store(s as u64, Ordering::Relaxed);
    }
    pub(super) fn load(&self) -> PlayerState {
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

pub(super) struct AtomicPosition {
    nanos: AtomicU64,
}

impl AtomicPosition {
    fn new() -> Self {
        Self {
            nanos: AtomicU64::new(0),
        }
    }
    pub(super) fn store(&self, d: Duration) {
        self.nanos.store(d.as_nanos() as u64, Ordering::Relaxed);
    }
    pub(super) fn load(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::Relaxed))
    }
}
