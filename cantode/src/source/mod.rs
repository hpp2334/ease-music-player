//! The byte-source layer.
//!
//! An [`AudioSource`] is anything cantode can decode from: a local file,
//! an HTTP range reader, an in-memory buffer, ... The trait is a small
//! extension of [`std::io::Read`] + [`std::io::Seek`] so that the
//! symphonia-based decoder (and `probe_metadata`) can consume it directly
//! without any async↔sync bridge inside the engine.
//!
//! Embedders implement this trait to plug their own byte source into
//! cantode. A ready-made [`MemoryAudioSource`] is provided for tests and
//! simple embedders; embedders fetching over a network should reach for
//! [`RemoteSource`], which takes a plain fetch closure and owns the
//! buffering, retries, and seek machinery itself.

pub mod memory;
pub mod remote;

pub use memory::MemoryAudioSource;
pub use remote::{RemoteSource, ReplyHandle};

use std::io::{Read, Seek};

/// A seekable byte source feeding the decoder.
///
/// `AudioSource` is intentionally **synchronous**. symphonia — cantode's
/// default decoder — is itself a sync `Read + Seek` consumer, so a sync
/// source lets the decoder read it directly with no adapter. Embedders
/// with an async byte source (e.g. an HTTP range client) should either
/// use the ready-made [`RemoteSource`] (which performs the bridging and
/// buffering itself) or bridge to sync on their side: the natural
/// pattern is to run the player's worker thread (or a `spawn_blocking`
/// task) and issue the async reads from a blocked sync call. See the
/// crate docs for the rationale.
///
/// Implementations must be `Send + Sync` because the source is moved onto
/// the player's dedicated worker thread.
pub trait AudioSource: Read + Seek + Send + Sync {
    /// Total content length in bytes, if known.
    ///
    /// Returning `None` is valid for streaming sources of unknown length
    /// (chunked HTTP, pipes); in that case duration/seek behaviour degrades
    /// gracefully — seeking past the buffered prefix is not possible and
    /// duration may be unavailable until the full stream is consumed.
    fn len(&self) -> Option<u64>;

    /// Whether the source has a known length of zero. Returns `false` for
    /// sources of unknown length (`len() == None`) — those *might* still
    /// yield bytes.
    fn is_empty(&self) -> bool {
        matches!(self.len(), Some(0))
    }

    /// Whether the source represents an effectively infinite stream.
    ///
    /// Defaults to `false`. Sources that never EOF (e.g. a live radio
    /// feed) should override this; cantode will avoid end-of-stream
    /// transitions in that case.
    fn is_infinite(&self) -> bool {
        false
    }
}
