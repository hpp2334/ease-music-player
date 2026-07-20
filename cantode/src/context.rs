//! [`PlayerContext`]: the shared-resource holder all players live in.
//!
//! A context owns:
//!
//! - the process-wide [`cpal::Host`] (cheap to clone, shared by value),
//! - a single [`DecoderFactory`] (shared by `Player::new` *and*
//!   `probe_metadata`, so probe and play agree on codecs),
//! - an optional global [`EventSink`] that sees every player's events,
//! - a registry of live players for diagnostics and bulk shutdown.
//!
//! `Player::new(&mut cx)` registers a player with the context (hence the
//! `&mut`); `probe_metadata(&cx, ...)` only reads the factory (hence the
//! `&`). [`Player`] does **not** borrow the context — it is `'static` and
//! can outlive it. `PlayerContext::Drop` best-effort-shuts-down any players
//! that are still registered (their own `Drop` is the primary path).

use std::sync::{Arc, Mutex, Weak};

use crate::{
    decoder::DecoderFactory, events::EventSink, CantodeError,
};

#[cfg(feature = "sink-cpal")]
use cpal::Host;

/// Configuration for a [`PlayerContext`].
#[derive(Default)]
pub struct PlayerContextConfig {
    /// cpal host to use. Defaults to [`cpal::default_host`] when `None`.
    #[cfg(feature = "sink-cpal")]
    pub host: Option<Host>,
    /// Decoder factory used by both `Player::new` and `probe_metadata`.
    /// Defaults to [`crate::SymphoniaDecoderFactory`] (the built-in
    /// symphonia-backed decoder).
    pub decoder_factory: Option<Arc<dyn DecoderFactory>>,
    /// Optional global event sink that receives events from every player
    /// created by this context.
    pub event_sink: Option<Arc<dyn EventSink>>,
}

/// Shared resources for one or more [`Player`]s.
///
/// Create one context per application (or per audio session) and hand out
/// players from it. The context is cheap to clone — but typical usage is a
/// single owned context on the audio thread, with `&mut PlayerContext`
/// borrowed only for `Player::new`.
pub struct PlayerContext {
    #[cfg(feature = "sink-cpal")]
    host: Host,
    decoder_factory: Arc<dyn DecoderFactory>,
    event_sink: Option<Arc<dyn EventSink>>,
    /// Registry of live players. Each entry is a `Weak` so a dropped
    /// player doesn't keep itself alive via the context.
    registry: Mutex<Vec<Weak<PlayerHandle>>>,
    /// Monotonic counter for worker-thread names (`cantode-player-N`).
    next_id: std::sync::atomic::AtomicU64,
}

/// Internal control handle for one player. Held `Arc`-strong inside the
/// [`Player`]; held `Weak` inside the context registry. The worker thread
/// holds no strong ref, so dropping the `Player` lets the registry entry
/// lazily evaporate.
pub(crate) struct PlayerHandle {
    pub(crate) shutdown: Mutex<Option<std::sync::mpsc::SyncSender<crate::player::Command>>>,
}

impl PlayerContext {
    /// Create a context with default configuration.
    pub fn new() -> Result<Self, CantodeError> {
        Self::with_config(PlayerContextConfig::default())
    }

    /// Create a context with explicit configuration.
    pub fn with_config(config: PlayerContextConfig) -> Result<Self, CantodeError> {
        #[cfg(feature = "sink-cpal")]
        let host = config.host.unwrap_or_else(cpal::default_host);

        let decoder_factory: Arc<dyn DecoderFactory> = config
            .decoder_factory
            .unwrap_or_else(default_decoder_factory);

        Ok(Self {
            #[cfg(feature = "sink-cpal")]
            host,
            decoder_factory,
            event_sink: config.event_sink,
            registry: Mutex::new(Vec::new()),
            next_id: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The cpal host backing this context.
    #[cfg(feature = "sink-cpal")]
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// The shared decoder factory. Used by [`Player::new`] and
    /// [`crate::probe_metadata`].
    pub fn decoder_factory(&self) -> &Arc<dyn DecoderFactory> {
        &self.decoder_factory
    }

    /// Global event sink, if one was configured.
    pub fn event_sink(&self) -> Option<&Arc<dyn EventSink>> {
        self.event_sink.as_ref()
    }

    /// Number of players created from this context that are still alive.
    ///
    /// Prunes lapsed weak references as a side effect.
    pub fn active_player_count(&self) -> usize {
        let mut reg = self.registry.lock().unwrap();
        reg.retain(|w| w.strong_count() > 0);
        reg.len()
    }

    // ----- package-private API used by Player::new -----

    /// Allocate the next worker-thread id/name.
    pub(crate) fn next_worker_name(&self) -> String {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("cantode-player-{id}")
    }

    /// Register a freshly-created player. Called by `Player::new`.
    pub(crate) fn register(&self, handle: Weak<PlayerHandle>) {
        let mut reg = self.registry.lock().unwrap();
        reg.retain(|w| w.strong_count() > 0);
        reg.push(handle);
    }
}

impl Drop for PlayerContext {
    fn drop(&mut self) {
        // Best-effort: signal any players that are still registered to shut
        // down. We can't `join` their threads here (the player owns the
        // join handle) — this is purely a "did you forget to drop your
        // players?" safety net.
        let reg = self.registry.lock().unwrap();
        for weak in reg.iter() {
            if let Some(h) = weak.upgrade()
                && let Some(tx) = h.shutdown.lock().unwrap().take()
            {
                // Send shutdown; ignore failure (worker already gone).
                let _ = tx.send(crate::player::Command::Shutdown);
            }
        }
    }
}

/// Default decoder factory used when `PlayerContextConfig::decoder_factory`
/// is `None`. Returns the built-in symphonia factory. Callers who want a
/// different decoder supply their own factory via `PlayerContextConfig`.
fn default_decoder_factory() -> Arc<dyn DecoderFactory> {
    Arc::new(crate::SymphoniaDecoderFactory::new())
}

// Re-export `Player` for the doc-link above to resolve regardless of
// module ordering.
#[allow(unused_imports)]
use crate::player::{Command as _PlayerCommandReexport, Player as _PlayerReexport};
