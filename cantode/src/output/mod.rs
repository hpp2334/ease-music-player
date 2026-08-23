//! The output (sink) layer (internal).
//!
//! [`AudioSink`] is an internal abstraction that consumes interleaved `f32`
//! PCM and pushes it to a destination. The only implementation shipped is
//! push-based API to cpal's callback-based output via a lock-free ring
//! buffer.
//!
//! `AudioSink` and `CpalSink` are deliberately `pub(crate)` — embedders
//! don't select or implement sinks; they just call [`Player`](crate::Player)
//! and cantode drives the system audio device on their behalf. There is no
//! "null" sink: tests and embedders run against the real cpal host. If the
//! host has no output device, `Player::load` fails with
//! [`CantodeError::NoOutputDevice`](crate::CantodeError::NoOutputDevice).

pub(crate) mod cpal;

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

    /// Drop all samples currently buffered in the sink without closing the
    /// stream. After `flush()` returns, the next `write()`d samples are the
    /// next ones the device callback will consume.
    ///
    /// Used on seek: the decoder jumps to a new position, but the sink may
    /// still be holding up to `buffer_secs` of pre-seek audio in its ring
    /// buffer. Without a flush, the listener hears ~2s of stale audio from
    /// the old position before the new position's samples arrive — and
    /// because the worker keeps decoding/pushing during that window, the
    /// stream becomes a discontinuous mix of old tail + new audio. Flushing
    /// synchronizes the buffer contents with the decoder position.
    fn flush(&mut self) -> crate::Result<()>;

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

/// Convert interleaved f32 PCM from `src_channels` to `dst_channels`,
/// writing into `out`.
///
/// Used by the player worker when the negotiated device channel count
/// differs from the decoder's channel count — e.g. a stereo source playing
/// through a genuinely mono-only device, or vice versa. The mix is simple
/// but correct enough for music playback:
///
/// - **Down-mix to mono**: every output sample is the average of the source
///   frame's channels. (Standard ITU down-mix would attenuate by 1/√2; we
///   use 1/N to avoid clipping on already-hot masters. Close enough for a
///   fallback path that should rarely fire.)
/// - **Up-mix from mono**: the single source sample is replicated onto
///   every output channel.
/// - **Equal channel count**: `out` is filled with a straight copy.
///
/// `out` must be sized for exactly `src.len() / src_channels * dst_channels`
/// samples. Returns the number of samples written.
pub(crate) fn remux_channels(
    src: &[f32],
    src_channels: u16,
    dst: &mut [f32],
    dst_channels: u16,
) -> usize {
    let src_ch = src_channels as usize;
    let dst_ch = dst_channels as usize;
    if src_ch == 0 || dst_ch == 0 {
        return 0;
    }
    let n_frames = src.len() / src_ch;
    let out_needed = n_frames * dst_ch;
    if dst.len() < out_needed {
        return 0;
    }
    if src_ch == dst_ch {
        dst[..out_needed].copy_from_slice(&src[..out_needed]);
        return out_needed;
    }
    if dst_ch == 1 {
        // Down-mix to mono: average channels per frame.
        for f in 0..n_frames {
            let mut sum = 0.0f32;
            for c in 0..src_ch {
                sum += src[f * src_ch + c];
            }
            dst[f] = sum / src_ch as f32;
        }
    } else if src_ch == 1 {
        // Up-mix mono → multi-channel: replicate.
        for f in 0..n_frames {
            let s = src[f];
            for c in 0..dst_ch {
                dst[f * dst_ch + c] = s;
            }
        }
    } else {
        // General mismatch (e.g. 6 → 2). Replicate-or-truncate channel by
        // channel. This branch is not high-fidelity but it never panics and
        // produces a listenable result for the rare device that requires it.
        for f in 0..n_frames {
            for c in 0..dst_ch {
                let s = if c < src_ch {
                    src[f * src_ch + c]
                } else {
                    src[f * src_ch]
                };
                dst[f * dst_ch + c] = s;
            }
        }
    }
    out_needed
}

#[cfg(test)]
mod tests {
    use super::remux_channels;

    #[test]
    fn remux_same_channel_count_is_identity() {
        let src = [0.1, 0.2, 0.3, 0.4]; // 2 frames × 2 channels
        let mut out = [0.0f32; 4];
        let n = remux_channels(&src, 2, &mut out, 2);
        assert_eq!(n, 4);
        assert_eq!(out, src);
    }

    #[test]
    fn remux_stereo_to_mono_averages_channels() {
        // Frame 0: (0.2, 0.6) → 0.4
        // Frame 1: (1.0, -1.0) → 0.0
        let src = [0.2, 0.6, 1.0, -1.0];
        let mut out = [0.0f32; 2];
        let n = remux_channels(&src, 2, &mut out, 1);
        assert_eq!(n, 2);
        assert!((out[0] - 0.4).abs() < 1e-6);
        assert!(out[1].abs() < 1e-6);
    }

    #[test]
    fn remux_mono_to_stereo_replicates() {
        let src = [0.25, -0.5];
        let mut out = [0.0f32; 4];
        let n = remux_channels(&src, 1, &mut out, 2);
        assert_eq!(n, 4);
        assert_eq!(out, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn remux_insufficient_output_returns_zero() {
        let src = [0.1, 0.2, 0.3, 0.4]; // 2 frames stereo → needs 4 mono out
        let mut out = [0.0f32; 1]; // too small
        let n = remux_channels(&src, 2, &mut out, 1);
        assert_eq!(n, 0);
    }
}
