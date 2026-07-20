//! The output (sink) layer (internal).
//!
//! [`AudioSink`] is an internal abstraction that consumes interleaved `f32`
//! PCM and pushes it to a destination. The only implementation shipped is
//! [`CpalSink`] (behind the `sink-cpal` feature), which bridges cantode's
//! push-based API to cpal's callback-based output via a lock-free ring
//! buffer.
//!
//! `AudioSink` and `CpalSink` are deliberately `pub(crate)` — embedders
//! don't select or implement sinks; they just call [`Player`](crate::Player)
//! and cantode drives the system audio device on their behalf. There is no
//! "null" sink: tests and embedders run against the real cpal host. If the
//! host has no output device, `Player::load` fails with
//! [`CantodeError::NoOutputDevice`](crate::CantodeError::NoOutputDevice).

#[cfg(feature = "sink-cpal")]
pub(crate) mod cpal;

#[cfg(feature = "sink-cpal")]
pub(crate) use self::cpal::CpalSink;

use std::time::Duration;

use crate::decoder::AudioFormat;

/// A destination for decoded PCM audio. Internal — not part of the public
/// API. See the module docs for why.
///
/// Lifecycle: a sink is created (via [`CpalSinkBuilder`]), then
/// [`AudioSink::start`] opens the underlying device/stream with a specific
/// [`AudioFormat`]; [`AudioSink::write`] is called repeatedly to feed PCM;
/// [`AudioSink::pause`] / [`AudioSink::resume`] gate output; and
/// [`AudioSink::stop`] closes the stream. Dropping the sink should be
/// equivalent to `stop()`.
///
/// The sink must be `Send` because the player worker owns it and may move
/// it onto its dedicated thread. It need not be `Sync`: only the worker
/// touches it after `start`.
pub(crate) trait AudioSink: Send {
    /// Open the underlying stream with the given format.
    ///
    /// The sink should be ready to receive `write` calls immediately on
    /// return, but it is acceptable for the actual device stream to start
    /// paused — `resume()` will start output. Returns the **actual**
    /// format the device is using (which may differ from `fmt` after
    /// device negotiation).
    #[allow(dead_code)] // unused when built without sink-cpal
    fn start(&mut self, fmt: AudioFormat) -> crate::Result<AudioFormat>;

    /// Close the underlying stream and release device resources.
    fn stop(&mut self) -> crate::Result<()>;

    /// Push interleaved f32 samples (`frames * format.channels` long) into
    /// the sink's internal buffer.
    ///
    /// Must not block indefinitely: sinks should either grow their buffer
    /// (bounded) or drop samples when full and report backpressure via the
    /// returned [`crate::Result`]. The player worker uses the return value
    /// to decide whether to enter `Buffering`.
    fn write(&mut self, frames: &[f32]) -> crate::Result<()>;

    /// Temporarily halt output without closing the stream. Idempotent.
    fn pause(&mut self) -> crate::Result<()>;

    /// Resume a paused sink. Idempotent.
    fn resume(&mut self) -> crate::Result<()>;

    /// Linear gain in `[0.0, ∞)`. `1.0` is unity; `0.0` is silent.
    fn set_volume(&mut self, vol: f32) -> crate::Result<()>;

    /// Current estimated end-to-end latency: time between a sample being
    /// `write`-en and it leaving the speakers. Reserved for use by the
    /// player to report accurate positions; not yet wired into the worker's
    /// position computation (a future improvement).
    #[allow(dead_code)]
    fn latency(&self) -> Duration;
}
