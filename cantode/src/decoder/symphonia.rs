//! symphonia-backed decoder.
//!
//! Implements [`Decoder`] / [`DecoderFactory`] on top of the pure-Rust
//! [`symphonia`][sym] crate. Supports every container/codec enabled by the
//! `symphonia` features in `Cargo.toml` (MP3, FLAC, Vorbis, WAV, AAC,
//! ISOMP4).
//!
//! PCM conversion follows symphonia's recommended pattern: a reusable
//! [`SampleBuffer::<f32>`] combined with
//! [`copy_interleaved_ref`][sym-copy] handles every native sample format
//! (u8..f64, including i24/u24) and de-interleaved planar layouts for us.
//!
//! [sym]: https://github.com/pdeljanov/Symphonia
//! [sym-copy]: symphonia::core::audio::SampleBuffer::copy_interleaved_ref

use std::time::Duration;

use symphonia::{
    core::{
        audio::{AudioBufferRef, Channels, SampleBuffer, SignalSpec},
        codecs::{CODEC_TYPE_NULL, CodecParameters, Decoder as SymDecoder, DecoderOptions},
        errors::Error as SymError,
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
        io::{MediaSource, MediaSourceStream},
        meta::{MetadataRevision, StandardTagKey, Tag},
        probe::Hint,
        units::{Time, TimeBase},
    },
    default::{get_codecs, get_probe},
};

use crate::{
    AudioSource, CantodeError, CoverArt, Metadata, Readiness,
    decoder::{AudioFormat, DecodedFrame, Decoder, DecoderFactory},
};

/// A [`DecoderFactory`] that opens sources via symphonia's default probe
/// and codec registries.
///
/// Stateless beyond its captured registries; cheap to clone (the
/// registries are process-global).
#[derive(Debug, Default, Clone)]
pub struct SymphoniaDecoderFactory;

impl SymphoniaDecoderFactory {
    /// Create a factory using symphonia's default codec/feature set.
    pub fn new() -> Self {
        Self
    }
}

impl DecoderFactory for SymphoniaDecoderFactory {
    fn open(&self, source: Box<dyn AudioSource>) -> crate::Result<Box<dyn Decoder>> {
        SymphoniaDecoder::open(source).map(|d| Box::new(d) as Box<dyn Decoder>)
    }
}

/// The shared handle to the boxed source. symphonia's
/// `MediaSourceStream` consumes the reader, so the decoder keeps this
/// clone to service [`Decoder::readiness`] /
/// [`Decoder::set_read_deadline`] after opening — the player's pump
/// needs them on the play path, and everything stays on the worker
/// thread (the `Mutex` is uncontended in practice).
type SharedSource = std::sync::Arc<std::sync::Mutex<Box<dyn AudioSource>>>;

/// Adapter that makes an [`AudioSource`] (our `Read + Seek + Send + Sync`
/// trait) satisfy symphonia's [`MediaSource`] trait (same shape, plus an
/// optional `byte_length` that we can delegate to `AudioSource::len`).
struct AudioSourceMediaSource(SharedSource);

impl std::io::Read for AudioSourceMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl std::io::Seek for AudioSourceMediaSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.lock().unwrap().seek(pos)
    }
}

impl MediaSource for AudioSourceMediaSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.0.lock().unwrap().len()
    }
}

/// A symphonia-backed [`Decoder`].
pub struct SymphoniaDecoder {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn SymDecoder>,
    track_id: u32,
    format: AudioFormat,
    time_base: TimeBase,
    metadata: Metadata,
    /// Reusable interleaved-f32 scratch buffer; grown as needed.
    sample_buf: SampleBuffer<f32>,
    /// Tracks how many frames the `sample_buf` is sized for so we can grow
    /// it when an unusually large packet arrives. In **frames** (channel
    /// groups), matching symphonia's notion of sample-buffer duration.
    sample_buf_capacity_frames: usize,
    /// The shared handle to the boxed source (also held inside `reader`'s
    /// `MediaSourceStream`) for readiness/deadline forwarding.
    source: SharedSource,
}

impl SymphoniaDecoder {
    fn open(source: Box<dyn AudioSource>) -> crate::Result<Self> {
        // Wrap our AudioSource in symphonia's MediaSource trait via the
        // adapter, keeping a shared handle so readiness/deadline calls
        // can still reach the source after the MediaSourceStream
        // consumes the reader.
        let source: SharedSource = std::sync::Arc::new(std::sync::Mutex::new(source));
        let boxed_msource: Box<dyn MediaSource> =
            Box::new(AudioSourceMediaSource(std::sync::Arc::clone(&source)));
        let mss = MediaSourceStream::new(boxed_msource, Default::default());
        let hint = Hint::new(); // sniff by content; we don't know the extension

        let mut probed = get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &Default::default(),
            )
            .map_err(map_probe_error)?;

        let mut reader = probed.format;

        // Collect metadata from BOTH places symphonia can store it:
        //   1. `probed.metadata` — revisions read by the probe *before* the
        //      format reader was instantiated (where ID3v2 tags live for
        //      MP3, since they precede the first audio frame).
        //   2. `reader.metadata()` — revisions stored *inside* the container
        //      (e.g. Vorbis comments, MP4 ilst, RIFF INFO).
        // Prefer the in-probe metadata; fall back to the in-container one.
        let probed_meta_rev: Option<MetadataRevision> = probed
            .metadata
            .get()
            .and_then(|mut m| m.skip_to_latest().cloned());
        let in_container_meta_rev: Option<MetadataRevision> =
            reader.metadata().skip_to_latest().cloned();
        let metadata_rev = probed_meta_rev.or(in_container_meta_rev);

        // Pick the first track with a known codec, preferring the
        // container's notion of a default track when it agrees. We keep a
        // reference (Track isn't Copy) and clone only the bits we need to
        // carry past the borrow on `reader`.
        let track_ref = reader
            .default_track()
            .filter(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .or_else(|| {
                reader
                    .tracks()
                    .iter()
                    .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            })
            .ok_or_else(|| CantodeError::UnsupportedFormat("no decodable track".into()))?;

        let codec_params: CodecParameters = track_ref.codec_params.clone();
        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| CantodeError::UnsupportedFormat("unknown sample rate".into()))?;
        let channels = codec_params
            .channels
            .map(|c| c.count() as u16)
            .ok_or_else(|| CantodeError::UnsupportedFormat("unknown channel layout".into()))?;
        let time_base = codec_params
            .time_base
            .ok_or_else(|| CantodeError::UnsupportedFormat("no time base".into()))?;

        let decoder = get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| CantodeError::Decode(format!("codec open failed: {e}")))?;

        let track_id = track_ref.id;
        let format = AudioFormat::new(channels, sample_rate);

        // Size the sample buffer for 1s of audio (plenty for any normal
        // frame). It grows on demand in `next_frame`. symphonia's
        // `SampleBuffer::new` takes a frame-count duration (raw `u64`).
        let initial_capacity_frames = sample_rate as u64;
        let chans = codec_params.channels.unwrap_or(Channels::FRONT_LEFT);
        let spec = SignalSpec::new(sample_rate, chans);
        let sample_buf = SampleBuffer::<f32>::new(initial_capacity_frames, spec);

        let metadata = build_metadata(metadata_rev.as_ref(), &codec_params, &format);

        Ok(Self {
            reader,
            decoder,
            track_id,
            format,
            time_base,
            metadata,
            sample_buf,
            sample_buf_capacity_frames: initial_capacity_frames as usize,
            source,
        })
    }
}

impl Decoder for SymphoniaDecoder {
    fn next_frame(&mut self) -> crate::Result<Option<DecodedFrame>> {
        loop {
            let packet = match self.reader.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(SymError::IoError(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // The play-path read deadline expired: the source is
                    // starved, not broken. Distinct from every other error
                    // so the pump can treat it as "needs data".
                    return Err(CantodeError::WouldBlock);
                }
                Err(SymError::ResetRequired) => continue,
                Err(e) => return Err(CantodeError::Decode(format!("read packet: {e}"))),
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(SymError::DecodeError(_)) => continue, // skip corrupt packet
                Err(e) => return Err(CantodeError::Decode(format!("decode packet: {e}"))),
            };

            let frames = decoded.frames();
            let ts = packet.ts();

            // Grow the sample buffer if this packet is larger than what we
            // currently have room for. `frames` is a channel-group count,
            // matching symphonia's notion of sample-buffer duration.
            if frames > self.sample_buf_capacity_frames {
                let chans = build_channels(self.format.channels);
                let spec = SignalSpec::new(self.format.sample_rate, chans);
                self.sample_buf = SampleBuffer::<f32>::new(frames as u64, spec);
                self.sample_buf_capacity_frames = frames;
            }

            self.sample_buf.copy_interleaved_ref(decoded);
            // `samples()` is only valid immediately after `copy_interleaved_ref`;
            // copy out so the buffer can be reused next iteration.
            let data = self.sample_buf.samples().to_vec();

            return Ok(Some(DecodedFrame {
                data,
                frames,
                timestamp: ts_to_duration(ts, self.time_base),
            }));
        }
    }

    fn seek(&mut self, target: Duration) -> crate::Result<Duration> {
        let seek_to = SeekTo::Time {
            time: Time::from(target),
            track_id: Some(self.track_id),
        };
        let sought = self
            .reader
            .seek(SeekMode::Accurate, seek_to)
            .map_err(|e| CantodeError::Decode(format!("seek: {e}")))?;
        // Flush decoder state so the next packet starts cleanly.
        self.decoder.reset();
        Ok(ts_to_duration(sought.required_ts, self.time_base))
    }

    fn format(&self) -> AudioFormat {
        self.format
    }

    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    fn readiness(&self) -> Readiness {
        self.source.lock().unwrap().readiness()
    }

    fn set_read_deadline(&mut self, deadline: Option<std::time::Duration>) {
        self.source.lock().unwrap().set_read_deadline(deadline);
    }
}

// Mark `AudioBufferRef` usage so the import is exercised even if symphonia's
// API surface shifts and the type is no longer inferred.
#[allow(dead_code)]
fn _audio_buffer_ref_in_scope(_b: AudioBufferRef) {}

// ----- helpers -----

fn ts_to_duration(ts: u64, tb: TimeBase) -> Duration {
    if tb.numer == 0 || tb.denom == 0 {
        return Duration::ZERO;
    }
    let secs = (ts * tb.numer as u64) as f64 / tb.denom as f64;
    Duration::from_secs_f64(secs)
}

fn map_probe_error(e: SymError) -> CantodeError {
    match e {
        SymError::Unsupported(msg) => CantodeError::UnsupportedFormat(msg.into()),
        other => CantodeError::UnsupportedFormat(format!("probe: {other}")),
    }
}

fn build_metadata(
    metadata_rev: Option<&MetadataRevision>,
    codec_params: &CodecParameters,
    format: &AudioFormat,
) -> Metadata {
    let tags: Vec<crate::Tag> = metadata_rev
        .map(|r| r.tags().iter().map(tag_to_pair).collect())
        .unwrap_or_default();

    // Cover art: symphonia stores it as a `Visual` on the metadata revision,
    // not as a binary tag. Grab the first one.
    let cover_art = metadata_rev
        .and_then(|r| r.visuals().first())
        .map(|v| CoverArt {
            data: v.data.to_vec(),
            mime: v.media_type.clone(),
        });

    let duration = codec_duration(codec_params);
    let total_samples = duration.and_then(|d| {
        let total = d.as_secs_f64() * format.sample_rate as f64;
        if total.is_finite() && total >= 0.0 {
            Some(total as u64)
        } else {
            None
        }
    });

    Metadata {
        format: *format,
        duration,
        total_samples,
        tags,
        cover_art,
    }
}

fn tag_to_pair(t: &Tag) -> crate::Tag {
    let key = t.std_key.map(std_key_name).unwrap_or_else(|| t.key.clone());
    crate::Tag {
        key,
        value: t.value.to_string(),
    }
}

fn std_key_name(k: StandardTagKey) -> String {
    format!("{k:?}")
}

/// Build a symphonia [`Channels`] bitmask for the first `n` channels.
///
/// Used when growing the sample buffer in `next_frame` — the exact channel
/// identities don't matter there, only the count.
fn build_channels(n: u16) -> Channels {
    // symphonia exposes FR, FL, FC, LFE, etc. as bitflags; we just take the
    // first N bits in declaration order so the count is correct.
    let mut bits = 0u32;
    for b in 0..(n.min(32) as u32) {
        bits |= 1u32 << b;
    }
    Channels::from_bits_truncate(bits)
}

fn codec_duration(params: &CodecParameters) -> Option<Duration> {
    let tb = params.time_base?;
    let n_frames = params.n_frames?;
    let secs = (n_frames * tb.numer as u64) as f64 / tb.denom as f64;
    if secs.is_finite() && secs > 0.0 {
        Some(Duration::from_secs_f64(secs))
    } else {
        None
    }
}
