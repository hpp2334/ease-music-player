//! Error types for the cantode audio engine.

use std::io;

use thiserror::Error;

/// The single error type returned by every cantode API.
///
/// Kept as one enum (rather than per-layer errors) so callers can match on a
/// single type regardless of which layer produced the failure. Variants are
/// deliberately non-`pub`-internally-tagged: this is a plain C-like enum so
/// it cheaply crosses thread boundaries and remains `Clone + Send + Sync`.
///
/// `Clone` is required because errors are surfaced via
/// [`PlayerEvent::Error`](crate::PlayerEvent), which is itself `Clone` so it
/// can be broadcast to multiple event-sink subscribers.
#[derive(Debug, Clone, Error)]
pub enum CantodeError {
    /// The caller invoked an operation that is illegal in the player's
    /// current state — e.g. `play()` while `Idle`, or `seek()` before
    /// `load()`. Carries the offending state for diagnostics.
    #[error("invalid player state for operation: {0}")]
    InvalidState(String),

    /// An [`AudioSource`] returned an I/O error. Stores the error message
    /// (and `io::Error` is not `Clone`, so we keep the string rather than
    /// wrapping it).
    #[error("audio source I/O error: {0}")]
    Source(String),

    /// The decoder rejected the byte stream or hit an unrecoverable decode
    /// failure (corrupt frame, unsupported codec, truncated file, ...).
    #[error("decode error: {0}")]
    Decode(String),

    /// The decoder could not identify a supported container/codec in the
    /// source. Usually raised during `load` / `probe_metadata`.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// A pluggable [`crate::AudioSink`] failed to open, start, or write.
    /// Carries a backend-specific message.
    #[error("audio sink error: {0}")]
    Sink(String),

    /// No output device is available on the current host. Raised when the
    /// default cpal output device cannot be enumerated.
    #[error("no output device available")]
    NoOutputDevice,

    /// The engine could not negotiate an output stream configuration that
    /// matches the decoded audio. Carries the requested format.
    #[error("stream configuration not supported: {0}")]
    StreamConfig(String),

    /// A command was issued to a player whose worker thread has already
    /// shut down (either because the player was dropped and re-used by
    /// mistake, or the worker panicked).
    #[error("player worker has exited")]
    WorkerExited,

    /// Catch-all for failures that don't fit a more specific variant.
    /// Prefer adding a specific variant over growing this one.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<io::Error> for CantodeError {
    fn from(e: io::Error) -> Self {
        CantodeError::Source(e.to_string())
    }
}

/// Shorthand `Result` alias used throughout the crate.
pub type Result<T, E = CantodeError> = std::result::Result<T, E>;
