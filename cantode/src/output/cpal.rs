//! cpal-backed hardware [`AudioSink`].
//!
//! [`CpalSink`] bridges cantode's push-based [`AudioSink::write`] to cpal's
//! callback-based output API via a lock-free SPSC ring buffer
//! ([`ringbuf`]). The producer side is fed from non-realtime threads (the
//! player worker); the consumer side is drained from cpal's realtime audio
//! thread. No allocations or blocking primitives appear on the consumer
//! side — that's the discipline cpal/AAudio/CoreAudio expect.
//!
//! On underflow (consumer empties faster than the producer fills), the
//! callback writes silence rather than blocking — preferable to glitching
//! the whole output subsystem or stalling the audio thread.

use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Sample, SizedSample, Stream, StreamConfig, SupportedStreamConfig,
};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapCons, HeapProd, HeapRb,
};

use crate::{decoder::AudioFormat, output::AudioSink, CantodeError};

/// Default ring-buffer capacity, expressed as seconds of audio at the
/// target sample rate. Two seconds gives the worker plenty of slack to
/// refill after a slow decode without underflowing.
const DEFAULT_BUFFER_SECS: f32 = 2.0;

/// Store the bit pattern of an f32 into an atomic so the realtime callback
/// can read the current gain without locks. A single relaxed load of the
/// bit pattern is sound on every platform cpal supports.
fn store_vol(slot: &AtomicU32, vol: f32) {
    slot.store(vol.to_bits(), Ordering::Relaxed);
}

fn load_vol(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

/// A hardware [`AudioSink`] backed by cpal.
///
/// Always targets the host's default output device and a 2-second ring
/// buffer. Cantode intentionally does not expose device/buffer
/// configuration — embedders get the system default, period. If a future
/// caller needs more, lift these into `PlayerContext` config rather than
/// widening this type's API.
pub(crate) struct CpalSink {
    stream: Option<Stream>,
    producer: Option<HeapProd<f32>>,
    /// Shared with the cpal callback so `set_volume` takes effect
    /// immediately without rebuilding the stream.
    volume: Arc<AtomicU32>,
    format: Option<AudioFormat>,
}

impl CpalSink {
    /// Construct a sink. The device itself is opened lazily by
    /// [`AudioSink::start`].
    pub(crate) fn new() -> Self {
        CpalSink {
            stream: None,
            producer: None,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            format: None,
        }
    }
}

impl Default for CpalSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSink for CpalSink {
    fn start(&mut self, fmt: AudioFormat) -> crate::Result<AudioFormat> {
        if self.stream.is_some() {
            return Ok(self.format.unwrap_or(fmt));
        }

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(CantodeError::NoOutputDevice)?;

        let desired_rate = fmt.sample_rate; // cpal::SampleRate is `u32`.
        let desired_channels = fmt.channels;
        let supported = pick_supported_config(&device, desired_rate, desired_channels)?;
        let stream_config = supported.config();
        let actual = AudioFormat::new(
            stream_config.channels as u16,
            stream_config.sample_rate, // also `u32`
        );

        // Size the ring buffer for `DEFAULT_BUFFER_SECS` of audio.
        let buf_secs = DEFAULT_BUFFER_SECS;
        let cap_samples = (((buf_secs * actual.sample_rate as f32) as usize)
            * actual.channels as usize)
        .next_power_of_two()
        .max(1024);
        let rb = HeapRb::<f32>::new(cap_samples);
        let (producer, consumer) = rb.split();

        let volume = self.volume.clone();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::F64 => build_stream::<f64>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::U32 => build_stream::<u32>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::I8 => build_stream::<i8>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::I64 => build_stream::<i64>(&device, &stream_config, consumer, volume)?,
            cpal::SampleFormat::U64 => build_stream::<u64>(&device, &stream_config, consumer, volume)?,
            other => {
                return Err(CantodeError::StreamConfig(format!(
                    "unsupported sample format: {other:?}"
                )))
            }
        };

        self.stream = Some(stream);
        self.producer = Some(producer);
        self.format = Some(actual);
        Ok(actual)
    }

    fn stop(&mut self) -> crate::Result<()> {
        self.stream = None;
        self.producer = None;
        self.format = None;
        Ok(())
    }

    fn write(&mut self, frames: &[f32]) -> crate::Result<()> {
        let Some(producer) = self.producer.as_mut() else {
            // Pre-start writes are silently discarded; callers don't need
            // to special-case initial buffering.
            return Ok(());
        };

        let mut to_push = frames;
        let total = frames.len();
        let mut pushed = 0usize;
        while !to_push.is_empty() {
            let n = producer.push_slice(to_push);
            if n == 0 {
                break;
            }
            pushed += n;
            to_push = &to_push[n..];
        }

        if pushed < total {
            tracing::debug!(
                dropped = total - pushed,
                "cpal sink ring buffer full; dropping samples"
            );
        }
        Ok(())
    }

    fn pause(&mut self) -> crate::Result<()> {
        if let Some(s) = self.stream.as_ref() {
            s.pause()
                .map_err(|e| CantodeError::Sink(format!("pause stream: {e}")))?;
        }
        Ok(())
    }

    fn resume(&mut self) -> crate::Result<()> {
        if let Some(s) = self.stream.as_ref() {
            s.play()
                .map_err(|e| CantodeError::Sink(format!("resume stream: {e}")))?;
        }
        Ok(())
    }

    fn set_volume(&mut self, vol: f32) -> crate::Result<()> {
        let clamped = vol.clamp(0.0, f32::MAX);
        store_vol(&self.volume, clamped);
        Ok(())
    }

    fn latency(&self) -> Duration {
        if let Some(p) = &self.producer {
            let occ = p.occupied_len();
            let fmt = self.format.unwrap_or(AudioFormat::new(2, 44_100));
            let samples_per_sec = (fmt.sample_rate as u64) * (fmt.channels as u64);
            if samples_per_sec > 0 {
                return Duration::from_secs_f64(occ as f64 / samples_per_sec as f64);
            }
        }
        Duration::ZERO
    }
}

impl Drop for CpalSink {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// ----- helpers -----

/// Pick the closest supported stream config for the desired sample rate
/// and channel count.
///
/// Preference order:
/// 1. A config whose sample-rate range contains `desired_rate` **and** whose
///    channel count equals `desired_channels`. (Exact match on both axes.)
/// 2. A config whose sample-rate range contains `desired_rate` (any
///    channels) — the caller will down/up-mix.
/// 3. The device's default output config.
///
/// Why channel count matters: on Android (AAudio via oboe/ndk-sys), the
/// device's *default* config reported by cpal is frequently **mono** even
/// when the device can actually sink stereo. If we accept that default for a
/// stereo source, the worker pushes 2-sample frames into the ring buffer
/// while the callback drains them as 1-sample frames — playback runs at 2×
/// speed and glitches. Requesting the source's channel count up front (with
/// a down/up-mix fallback for genuinely mono-only devices) is the fix.
fn pick_supported_config(
    device: &cpal::Device,
    desired_rate: u32,
    desired_channels: u16,
) -> crate::Result<SupportedStreamConfig> {
    if let Ok(mut configs) = device.supported_output_configs() {
        // (1) Exact match on rate + channels.
        if let Some(c) = configs.find(|c| {
            c.channels() == desired_channels
                && c.min_sample_rate() <= desired_rate
                && desired_rate <= c.max_sample_rate()
        }) {
            return Ok(c.with_sample_rate(desired_rate));
        }
    }
    if let Ok(mut configs) = device.supported_output_configs()
        && let Some(c) = configs.find(|c| {
            c.min_sample_rate() <= desired_rate && desired_rate <= c.max_sample_rate()
        })
    {
        return Ok(c.with_sample_rate(desired_rate));
    }
    device
        .default_output_config()
        .map_err(|e| CantodeError::StreamConfig(format!("default output config: {e}")))
}

/// Build a cpal output stream of the given device sample type `T`.
///
/// The callback drains the ring buffer's consumer into `out`, applying the
/// current volume (read atomically) and converting f32 → `T` via
/// [`cpal::Sample::from_sample`]. On underflow it writes silence
/// (`T::EQUILIBRIUM`) rather than blocking — standard RT-safety discipline.
fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    consumer: HeapCons<f32>,
    volume: Arc<AtomicU32>,
) -> crate::Result<Stream>
where
    T: SizedSample + cpal::FromSample<f32> + Send + 'static,
{
    let mut consumer = consumer;
    let err_fn = |err: cpal::StreamError| tracing::error!("cpal stream error: {err}");

    let stream = device
        .build_output_stream::<T, _, _>(
            config,
            move |out: &mut [T], _info: &cpal::OutputCallbackInfo| {
                let vol = load_vol(&volume);
                drain_into(&mut consumer, out, vol);
            },
            err_fn,
            None,
        )
        .map_err(|e| CantodeError::Sink(format!("build stream: {e}")))?;
    Ok(stream)
}

/// Drain the ring buffer into `out`, applying gain in f32 space, then
/// converting to the device sample type. Silence on underflow.
fn drain_into<T: Sample + cpal::FromSample<f32>>(
    consumer: &mut HeapCons<f32>,
    out: &mut [T],
    vol: f32,
) {
    // Stack scratch avoids per-call allocation while keeping the pop loop
    // reasonably chunky.
    let mut tmp = [0f32; 256];
    let mut filled = 0;
    while filled < out.len() {
        let want = (out.len() - filled).min(tmp.len());
        let got = consumer.pop_slice(&mut tmp[..want]);
        if got == 0 {
            break;
        }
        for &s in &tmp[..got] {
            out[filled] = T::from_sample::<f32>(s * vol);
            filled += 1;
        }
    }
    // Underflow tail → silence.
    for slot in &mut out[filled..] {
        *slot = T::EQUILIBRIUM;
    }
}
