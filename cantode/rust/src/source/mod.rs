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
//! simple embedders; embedders streaming over a network should implement
//! [`RemoteAudioSource`] (the long-lived session trait) and wrap it in a
//! [`BufferedSource`], which owns the buffering, demand scheduling,
//! retries, and seek machinery itself.

pub mod buffered;
pub mod memory;

pub use buffered::{BufferedSource, Pushed, RemoteAudioSource, StreamReply};
pub use memory::MemoryAudioSource;

use std::io::{Read, Seek};
use std::time::Duration;

/// Whether an [`AudioSource`] currently has bytes at the read cursor, or
/// would park a reader.
///
/// An advisory signal for the player's play path: the worker consults it
/// before each decode step so a starved network window surfaces as
/// [`PlayerState::Buffering`](crate::PlayerState::Buffering) instead of a
/// frozen-but-`Playing` position. Sources that can park (`BufferedSource`)
/// override it; everything else defaults to [`Readiness::Ready`] and keeps
/// the classic park semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Readiness {
    /// A read at the cursor will be satisfied without parking (data is
    /// buffered, or the source is at a terminal — EOF / error — where the
    /// read returns immediately).
    #[default]
    Ready,
    /// The cursor sits at the end of the buffered window while the source
    /// is still alive — a read would park until data arrives.
    NeedsData,
}

/// The contiguous buffered byte window of a buffering source.
///
/// Reported by [`AudioSource::buffered_range`] — sources that maintain a
/// readahead window (see [`BufferedSource`](crate::BufferedSource))
/// describe it in absolute byte offsets so an embedder can render
/// "buffered amount" UI. Non-buffering sources report `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferedRange {
    /// Absolute byte offset where the contiguous window starts. Only
    /// moves forward (consumed-prefix eviction); a seek resets it to the
    /// seek target.
    pub start: u64,
    /// Absolute byte offset of the window's end (exclusive) — the
    /// buffered frontier.
    pub end: u64,
    /// Total resource length in bytes, when known (`Content-Length`,
    /// ...). `None` for streams of unknown length.
    pub total: Option<u64>,
}

/// A seekable byte source feeding the decoder.
///
/// `AudioSource` is intentionally **synchronous**. symphonia — cantode's
/// default decoder — is itself a sync `Read + Seek` consumer, so a sync
/// source lets the decoder read it directly with no adapter. Embedders
/// with an async byte source (e.g. an HTTP range client) should implement
/// [`RemoteAudioSource`] and wrap it in [`BufferedSource`] (which performs
/// the bridging and buffering itself), or bridge to sync on their side:
/// the natural pattern is to run the player's worker thread (or a
/// `spawn_blocking` task) and issue the async reads from a blocked sync
/// call. See the crate docs for the rationale.
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

    /// Advisory: can a read at the current cursor be satisfied without
    /// parking? See [`Readiness`]. Default [`Readiness::Ready`] — sources
    /// that park (network windows) override this to opt into the player's
    /// `Buffering` behavior.
    fn readiness(&self) -> Readiness {
        Readiness::Ready
    }

    /// The contiguous buffered window around the read cursor, for
    /// embedder "buffered amount" UI. See [`BufferedRange`]. Default
    /// `None` — sources that maintain a readahead window
    /// ([`BufferedSource`](crate::BufferedSource)) override this.
    fn buffered_range(&self) -> Option<BufferedRange> {
        None
    }

    /// Bound the parks of subsequent `Read` calls: while
    /// a deadline is set, a read that finds no data at the cursor returns
    /// an `io::ErrorKind::WouldBlock` error once the
    /// deadline elapses instead of parking indefinitely. `None` (the
    /// default resting state) parks until data arrives.
    ///
    /// Used by the player's play path (arm before a decode step, clear
    /// after) so a starved read surfaces as "needs data" instead of
    /// wedging the pump. Sources that cannot park ignore it.
    fn set_read_deadline(&mut self, _deadline: Option<Duration>) {}
}
