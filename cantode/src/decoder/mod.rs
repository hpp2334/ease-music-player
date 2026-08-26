//! The decoder layer.
//!
//! A [`Decoder`] pulls compressed packets out of an [`AudioSource`] and
//! yields decoded PCM frames. The built-in implementation,
//! [`SymphoniaDecoder`], is built on the pure-Rust [`symphonia`][sym] crate
//! and supports MP3, FLAC, Vorbis, WAV, AAC, and ISOMP4 out of the box.
//!
//! Embedders that want to substitute their own decoder — e.g. to delegate
//! AAC to Android `MediaCodec` — implement [`Decoder`] + [`DecoderFactory`]
//! and hand the factory to a [`PlayerContext`](crate::PlayerContext).
//!
//! [sym]: https://github.com/pdeljanov/Symphonia

pub mod symphonia;

pub use self::symphonia::{SymphoniaDecoder, SymphoniaDecoderFactory};

use std::time::Duration;

use crate::{AudioSource, Metadata};

/// PCM format produced by a [`Decoder`].
///
/// Cantode's internal PCM convention is **interleaved `f32` samples** in
/// the range `[-1.0, 1.0]`. Decoders are responsible for converting from
/// their native sample format (signed/unsigned int, float, planar/interleaved)
/// into this canonical form — see [`DecodedFrame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    /// Number of channels (1 = mono, 2 = stereo, ...).
    pub channels: u16,
    /// Samples per second, per channel (e.g. 44100, 48000).
    pub sample_rate: u32,
}

impl AudioFormat {
    /// Construct a new format descriptor.
    pub const fn new(channels: u16, sample_rate: u32) -> Self {
        Self {
            channels,
            sample_rate,
        }
    }
}

impl Default for AudioFormat {
    /// Defaults to "unknown" — a single channel at 0 Hz. Useful as a
    /// placeholder before the decoder has reported the actual format.
    fn default() -> Self {
        Self::new(1, 0)
    }
}

/// One chunk of decoded PCM audio.
///
/// `data` is **interleaved** `f32` samples in `[-1.0, 1.0]`:
/// for stereo, the layout is `[L0, R0, L1, R1, …]`.
///
/// - `frames` is the number of channel-groups (so `data.len() == frames *
///   format.channels`). The name "frame" matches audio-engine conventions
///   (one sample per channel at one time instant), not video.
/// - `timestamp` is the time of the **first** sample in this chunk,
///   relative to the start of the source.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Interleaved f32 samples, range approximately `[-1.0, 1.0]`.
    pub data: Vec<f32>,
    /// Number of channel-groups in `data`.
    pub frames: usize,
    /// Wall-clock time of the first sample in `data`.
    pub timestamp: Duration,
}

impl DecodedFrame {
    /// Construct an empty frame at the given timestamp.
    pub fn empty_at(timestamp: Duration) -> Self {
        Self {
            data: Vec::new(),
            frames: 0,
            timestamp,
        }
    }
}

/// A decoder pulls PCM frames out of an [`AudioSource`].
///
/// `Decoder` is **not** `Sync`: a decoder holds mutable internal state
/// (bitstream buffers, codec scratch space) and is driven exclusively by
/// the player worker thread that owns it. `Send` is sufficient.
pub trait Decoder: Send {
    /// Decode the next chunk of PCM audio.
    ///
    /// Returns:
    /// - `Ok(Some(frame))` — a chunk of decoded audio.
    /// - `Ok(None)` — end of stream reached; further `next_frame` calls
    ///   are allowed and should keep returning `Ok(None)`.
    /// - `Err(_)` — a decode error. Non-fatal decode errors (e.g. a single
    ///   corrupt packet) may be recoverable; callers decide whether to
    ///   retry or surface the error.
    fn next_frame(&mut self) -> crate::Result<Option<DecodedFrame>>;

    /// Seek to `target` relative to the source start.
    ///
    /// Returns the timestamp actually seeked to, which may differ from
    /// `target` because most codecs can only seek to packet boundaries.
    /// On return, the next [`Decoder::next_frame`] call yields audio
    /// starting at (approximately) the returned timestamp.
    fn seek(&mut self, target: Duration) -> crate::Result<Duration>;

    /// The format of the audio this decoder produces.
    ///
    /// Constant for the lifetime of the decoder (a given source's format
    /// does not change mid-stream in any supported container).
    fn format(&self) -> AudioFormat;

    /// Parsed metadata for the currently-loaded source.
    ///
    /// Available immediately after the decoder is opened — no need to
    /// decode any frames first. May be partial for sources whose
    /// container stores metadata at the end (e.g. ID3v1); in that case
    /// fields will simply be `None` / empty.
    fn metadata(&self) -> &Metadata;

    /// Advisory: does the underlying source have bytes at its read
    /// cursor, or would it park? Forwards
    /// [`AudioSource::readiness`] when the
    /// decoder can still reach its source; default
    /// [`Readiness::Ready`](crate::Readiness::Ready).
    fn readiness(&self) -> crate::Readiness {
        crate::Readiness::Ready
    }

    /// Arm/clear the play-path read deadline on the underlying source
    /// (see [`AudioSource::set_read_deadline`]).
    /// The player's pump arms a short deadline around each decode step so
    /// a starved source surfaces as [`crate::CantodeError::WouldBlock`]
    /// instead of parking the worker. Default no-op.
    fn set_read_deadline(&mut self, _deadline: Option<std::time::Duration>) {}
}

/// Constructs [`Decoder`]s from [`AudioSource`]s.
///
/// The factory indirection exists so a `PlayerContext` can share codec
/// registries / probe state across many players and across
/// `probe_metadata` calls, instead of rebuilding them per source.
pub trait DecoderFactory: Send + Sync {
    /// Open `source` for decoding. The decoder takes ownership of the
    /// source (most decoders need to own the reader to satisfy `Send`).
    fn open(&self, source: Box<dyn AudioSource>) -> crate::Result<Box<dyn Decoder>>;
}
