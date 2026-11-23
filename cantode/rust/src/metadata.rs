//! Metadata types and the no-playback [`probe_metadata`] entry point.
//!
//! [`probe_metadata`] decodes just enough of a source to read tags,
//! duration, and cover art — it never opens an output device. It reuses
//! the [`PlayerContext`]'s shared [`DecoderFactory`] so that "probe" and
//! "play" agree on codecs.

use std::time::Duration;

use crate::{AudioSource, PlayerContext, decoder::AudioFormat};

/// Parsed metadata describing an audio source.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    /// Decoded PCM format. For most containers this is known from the
    /// header alone.
    pub format: AudioFormat,
    /// Total playback duration, if the container advertises it.
    pub duration: Option<Duration>,
    /// Total samples per channel, if known. Derived from `duration` and
    /// `format.sample_rate` when the container doesn't list it directly.
    pub total_samples: Option<u64>,
    /// Free-form tag pairs (key, value). Keys are typically
    /// symphonia-standard tag names (`TrackTitle`, `Artist`, `Album`,
    /// `AlbumArtist`, ...).
    pub tags: Vec<Tag>,
    /// Embedded cover art, if any.
    pub cover_art: Option<CoverArt>,
}

/// A single free-form metadata tag (key/value pair).
///
/// Stored as a small struct rather than a tuple so the type crosses
/// foreign-binding boundaries (e.g. UniFFI) cleanly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tag {
    /// Tag key — typically a symphonia-standard name (`TrackTitle`,
    /// `Artist`, ...).
    pub key: String,
    /// Tag value.
    pub value: String,
}

/// Embedded picture metadata (typically ID3 APIC or FLAC
/// `METADATA_BLOCK_PICTURE`).
#[derive(Debug, Clone)]
pub struct CoverArt {
    /// Raw image bytes (PNG / JPEG / ...).
    pub data: Vec<u8>,
    /// MIME type, e.g. `image/png`, `image/jpeg`.
    pub mime: String,
}

/// Probe a source for metadata without playing it.
///
/// Opens the source via the context's shared [`DecoderFactory`], reads its
/// metadata, and returns it. **Does not** decode any audio frames and does
/// not open any output device — safe to call on background threads for
/// batch scanning.
///
/// Takes ownership of `source` for consistency with
/// [`DecoderFactory::open`]; callers that want to keep the source can
/// `Box::clone` their reader or re-open it after probing.
pub fn probe_metadata(cx: &PlayerContext, source: Box<dyn AudioSource>) -> crate::Result<Metadata> {
    let decoder = cx.decoder_factory().open(source)?;
    Ok(decoder.metadata().clone())
}
