//! Player events and the [`EventSink`] abstraction.
//!
//! The player worker emits [`PlayerEvent`]s as it runs. By default they
//! are dropped; callers attach an [`EventSink`] — either on
//! [`PlayerContext`](crate::PlayerContext) (global, sees every player) or
//! per-player via [`PlayerConfig`](crate::PlayerConfig) — to receive them.
//!
//! [`ChannelEventSink`] is a ready-made implementation that fans events
//! out to multiple receivers via bounded mpsc channels.

use std::sync::{Arc, Mutex, mpsc};

use crate::{CantodeError, Metadata, state::PlayerState};

/// Something that happened inside a player.
///
/// Emitted to the configured [`EventSink`]s. These are the only
/// notifications a UI / caller receives about playback progress.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// The player's [`PlayerState`] changed.
    StateChanged(PlayerState),
    /// Playback position advanced. Throttled internally to ~10 Hz so
    /// callers don't need to do their own rate-limiting.
    PositionChanged(std::time::Duration),
    /// Metadata for the loaded source is available. Emitted once per
    /// successful [`Player::load`](crate::Player::load).
    MetadataReady(Metadata),
    /// A non-fatal or fatal error occurred. Fatal errors also flip the
    /// player to [`PlayerState::Error`].
    Error(CantodeError),
    /// The loaded source has been played to its end — with a
    /// position-tracking sink, after the buffered tail has actually
    /// sounded. Emitted exactly once per `load` (subsequent seeks back
    /// into the audio will not re-emit).
    Ended,
}

/// A sink for [`PlayerEvent`]s.
///
/// Implementations must be `Send + Sync` because events are emitted from
/// the player's dedicated worker thread while the sink handle is held on
/// the caller's thread. [`emit`](EventSink::emit) must be non-blocking;
/// long-running consumers should buffer events and process them elsewhere.
pub trait EventSink: Send + Sync {
    /// Receive one event. Must be non-blocking.
    fn emit(&self, event: PlayerEvent);
}

/// An [`EventSink`] that ignores everything. The default when no sink is
/// configured.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit(&self, _event: PlayerEvent) {}
}

/// An [`EventSink`] that fans events out to N subscribers via bounded mpsc
/// channels. Implemented on `std::sync::mpsc` so the crate stays
/// runtime-agnostic.
///
/// Each subscriber gets its own bounded queue of `cap` events. If a
/// subscriber's queue is full when a new event arrives, the new event is
/// dropped for that subscriber (slow-subscriber isolation). Subscribers
/// whose receiver was dropped are pruned on each emit.
pub struct ChannelEventSink {
    inner: Arc<ChannelEventSinkInner>,
}

struct ChannelEventSinkInner {
    /// Per-subscriber channel capacity applied by `subscribe`.
    default_cap: usize,
    subscribers: Mutex<Vec<mpsc::SyncSender<PlayerEvent>>>,
}

impl ChannelEventSink {
    /// Create a sink. `cap` is the per-subscriber channel capacity used
    /// by [`subscribe`](Self::subscribe).
    pub fn new(cap: usize) -> Self {
        Self {
            inner: Arc::new(ChannelEventSinkInner {
                default_cap: cap.max(1),
                subscribers: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Subscribe to events with this sink's default per-subscriber capacity.
    /// Returns a receiver that yields events until all clones of this sink
    /// are dropped (and any in-flight events drain).
    pub fn subscribe(&self) -> mpsc::Receiver<PlayerEvent> {
        let (tx, rx) = mpsc::sync_channel(self.inner.default_cap);
        self.inner.subscribers.lock().unwrap().push(tx);
        rx
    }

    /// Subscribe with an explicit per-subscriber capacity that overrides
    /// the sink's default.
    pub fn subscribe_with_cap(&self, cap: usize) -> mpsc::Receiver<PlayerEvent> {
        let (tx, rx) = mpsc::sync_channel(cap.max(1));
        self.inner.subscribers.lock().unwrap().push(tx);
        rx
    }
}

impl EventSink for ChannelEventSink {
    fn emit(&self, event: PlayerEvent) {
        let mut subs = self.inner.subscribers.lock().unwrap();
        // `retain` drops subscribers whose receiver is gone OR whose queue
        // is full (we treat a full queue as "too slow; drop this event").
        subs.retain(|tx| tx.try_send(event.clone()).is_ok());
    }
}
