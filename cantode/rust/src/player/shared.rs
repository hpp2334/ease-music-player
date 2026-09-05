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
//! [`Loaded`](super::session::Loaded) session alone, `set_duration` and
//! `set_buffered_range` by the worker alone, and
//! `reset_session_observables` by `Loaded::drop` alone. The transition
//! log is likewise appended by `set_state` alone — one entry per real
//! state change.

use std::collections::VecDeque;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use crate::state::PlayerState;
use crate::BufferedRange;

/// How many recent transitions the log keeps. The poller drains at 10 Hz
/// and transitions are rare (a handful per track), so 16 is generous; if
/// a client ever falls further behind, it detects the seq discontinuity
/// and trusts the current state instead.
const TRANSITION_LOG_CAP: usize = 16;

pub(super) struct SharedStatus {
    /// Mirrored by the machine's commit — its only writer.
    state: AtomicState,
    /// Last decoded-frame timestamp; reset by `Loaded::drop`.
    position: AtomicPosition,
    /// Duration of the loaded source; set by the worker on load success,
    /// reset by `Loaded::drop`.
    duration: Mutex<Option<Duration>>,
    /// Contiguous buffered window of the loaded source (when it maintains
    /// one); mirrored by the worker on its ticks, reset by `Loaded::drop`.
    buffered: Mutex<Option<BufferedRange>>,
    /// Monotonic transition counter — bumped once per real state change
    /// (the machine only commits on change), never reset.
    transition_seq: AtomicU64,
    /// Ring of recent `(seq, state)` transitions, appended by
    /// [`SharedStatus::set_state`]. Lets a sampling client (10 Hz UI
    /// poll) recover sub-tick excursions — e.g. a fast
    /// `Loading → Playing` that completed between two samples.
    transitions: Mutex<VecDeque<(u64, PlayerState)>>,
}

impl SharedStatus {
    pub(super) fn new() -> Self {
        Self {
            state: AtomicState::new(PlayerState::Idle),
            position: AtomicPosition::new(),
            duration: Mutex::new(None),
            buffered: Mutex::new(None),
            transition_seq: AtomicU64::new(0),
            transitions: Mutex::new(VecDeque::new()),
        }
    }

    /// The externally-observable state. Lock-free.
    pub(super) fn state(&self) -> PlayerState {
        self.state.load()
    }

    /// Mirror a committed phase. [`Machine::commit`] only. Called exactly
    /// once per real state change, so this is also the single append
    /// point of the transition log.
    pub(super) fn set_state(&self, s: PlayerState) {
        self.state.store(s);
        let seq = self.transition_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let mut log = self.transitions.lock().unwrap();
        if log.len() == TRANSITION_LOG_CAP {
            log.pop_front();
        }
        log.push_back((seq, s));
    }

    /// Current transition seq + the transitions with `seq > after`, in
    /// order. If the first returned entry's seq is not `after + 1`, the
    /// caller missed entries (log overrun) and should trust the current
    /// state rather than replay the partial history.
    pub(super) fn transitions_since(&self, after: u64) -> (u64, Vec<(u64, PlayerState)>) {
        let log = self.transitions.lock().unwrap();
        let entries = log
            .iter()
            .filter(|(seq, _)| *seq > after)
            .copied()
            .collect();
        (self.transition_seq.load(Ordering::Relaxed), entries)
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

    /// The source's contiguous buffered window, when it maintains one.
    pub(super) fn buffered_range(&self) -> Option<BufferedRange> {
        *self.buffered.lock().unwrap()
    }

    /// Mirror the source's buffered window (worker's periodic refresh —
    /// see the worker's ticks). Worker only.
    pub(super) fn set_buffered_range(&self, r: Option<BufferedRange>) {
        *self.buffered.lock().unwrap() = r;
    }

    /// Reset the session-scoped observables (position, duration,
    /// buffered window). `Loaded::drop` only — session death is the one
    /// reset point.
    pub(super) fn reset_session_observables(&self) {
        self.position.store(Duration::ZERO);
        *self.duration.lock().unwrap() = None;
        *self.buffered.lock().unwrap() = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_log_records_seq_and_drains_since() {
        let shared = SharedStatus::new();
        shared.set_state(PlayerState::Loading);
        shared.set_state(PlayerState::Playing);

        let (seq, entries) = shared.transitions_since(0);
        assert_eq!(seq, 2);
        assert_eq!(
            entries,
            vec![(1, PlayerState::Loading), (2, PlayerState::Playing)]
        );

        // Incremental drain: only the newer entry comes back.
        let (_, entries) = shared.transitions_since(1);
        assert_eq!(entries, vec![(2, PlayerState::Playing)]);

        // Nothing new: empty, not a repeat.
        let (seq, entries) = shared.transitions_since(2);
        assert_eq!(seq, 2);
        assert!(entries.is_empty());
    }

    #[test]
    fn transition_log_evicts_oldest_beyond_cap() {
        let shared = SharedStatus::new();
        for _ in 0..(TRANSITION_LOG_CAP + 4) {
            shared.set_state(PlayerState::Paused);
            shared.set_state(PlayerState::Playing);
        }
        let (seq, entries) = shared.transitions_since(0);
        // Seq keeps counting; the ring kept only the newest CAP entries.
        assert_eq!(entries.len(), TRANSITION_LOG_CAP);
        assert_eq!(entries.last().copied().unwrap().0, seq);
        // Overrun is detectable: the oldest surviving seq is not 1.
        assert_ne!(entries.first().copied().unwrap().0, 1);
    }
}
