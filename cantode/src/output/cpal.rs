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
    /// Flush generation counter, shared with the cpal callback.
    ///
    /// When `flush()` is called, the worker bumps this counter. The callback
    /// remembers the last-seen value; on entry, if it differs, the callback
    /// drains-and-discards the entire ring buffer before producing output.
    /// This is the safe (no `unsafe`) way to make subsequent `write()`s the
    /// next thing the device plays — required on seek, otherwise the device
    /// plays up to `buffer_secs` of pre-seek audio before the new position
    /// arrives, producing a discontinuous mix.
    flush_gen: Arc<AtomicU32>,
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
            flush_gen: Arc::new(AtomicU32::new(0)),
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
            stream_config.sample_rate,
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
        let flush_gen = self.flush_gen.clone();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::F64 => build_stream::<f64>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::U32 => build_stream::<u32>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::I8 => build_stream::<i8>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::I64 => build_stream::<i64>(&device, &stream_config, consumer, volume, flush_gen)?,
            cpal::SampleFormat::U64 => build_stream::<u64>(&device, &stream_config, consumer, volume, flush_gen)?,
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

        // Backpressure: when the ring buffer is full, the consumer (cpal
        // callback) hasn't drained enough yet. BLOCK here for a bounded
        // time instead of dropping samples — otherwise the worker would
        // burn through the source audio (dropping most frames) while the
        // position counter races ahead and the audio output becomes a
        // fragmented mix of source positions.
        //
        // We poll with a short sleep (no condvar — the cpal callback is RT
        // and must not be expected to signal a waiter). 2ms is fine-grained
        // enough to keep the buffer topped up without busy-spinning.
        const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(2);
        // If we wait this long without ANY progress, give up on the
        // remainder (likely a stream stall / device disconnect).
        const MAX_STALL: std::time::Duration = std::time::Duration::from_secs(2);

        let mut stall_total = std::time::Duration::ZERO;
        while !to_push.is_empty() {
            let n = producer.push_slice(to_push);
            if n == 0 {
                if stall_total >= MAX_STALL {
                    tracing::warn!(
                        remaining = to_push.len(),
                        waited_ms = stall_total.as_millis() as u64,
                        "sink write stalled beyond MAX_STALL; dropping remainder"
                    );
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
                stall_total += POLL_INTERVAL;
                continue;
            }
            pushed += n;
            to_push = &to_push[n..];
            stall_total = std::time::Duration::ZERO;
        }

        if pushed < total {
            tracing::debug!(
                dropped = total - pushed,
                "cpal sink dropped samples after MAX_STALL"
            );
        }

        Ok(())
    }

    fn flush(&mut self) -> crate::Result<()> {
        // Bump the flush generation. The cpal callback will detect the
        // change on its next invocation and discard everything currently
        // in the ring buffer before producing output, so subsequent
        // `write()`s are the next samples the device plays.
        //
        // We DON'T touch the producer side here: any samples pushed before
        // this call but not yet consumed are discarded by the callback; any
        // samples the worker pushes after `flush()` returns race with the
        // callback's discard but will be correctly consumed because the
        // discard runs at callback entry with a single atomic load.
        self.flush_gen.fetch_add(1, Ordering::Release);
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

/// Pick the closest supported stream config for the desired sample rate,
/// channel count, and sample format.
///
/// Preference order (each step tries F32 first, then falls back to other
/// formats):
/// 1. A config whose sample-rate range contains `desired_rate` **and** whose
///    channel count equals `desired_channels` **and** whose sample format is
///    F32. (Exact match on all three axes — what we want.)
/// 2. Same as (1) but accepting any sample format the device lists.
/// 3. A config whose sample-rate range contains `desired_rate` and channels
///    match (any format).
/// 4. A config whose sample-rate range contains `desired_rate` (any channels,
///    any format) — the caller will down/up-mix.
/// 5. The device's default output config.
///
/// Two historical bugs drive the F32 preference:
///
/// - **Channel mismatch (2× speed):** on Android (AAudio via oboe/ndk-sys),
///   the device's *default* config reported by cpal is frequently **mono**
///   even when the device can actually sink stereo. Accepting that default
///   for a stereo source pushes 2-sample frames into the ring buffer while
///   the callback drains them as 1-sample frames.
///
/// - **Format mismatch (2× speed + distortion):** cpal's Android backend
///   sometimes reports `SampleFormat::I16` as supported, but AAudio actually
///   opens the stream as **PCM_FLOAT** regardless. cpal then drives an i16
///   stream callback while AAudio reads f32 → 2-byte writes consumed as
///   4-byte reads → buffer exhaustion rate doubles and sample interpretation
///   is wrong. Forcing F32 makes cpal and AAudio agree on the wire format.
fn pick_supported_config(
    device: &cpal::Device,
    desired_rate: u32,
    desired_channels: u16,
) -> crate::Result<SupportedStreamConfig> {
    // (1) Exact match on rate + channels + F32.
    if let Ok(mut configs) = device.supported_output_configs()
        && let Some(c) = configs.find(|c| {
            c.channels() == desired_channels
                && c.sample_format() == cpal::SampleFormat::F32
                && c.min_sample_rate() <= desired_rate
                && desired_rate <= c.max_sample_rate()
        })
    {
        return Ok(c.with_sample_rate(desired_rate));
    }
    // (2)/(3) Rate + channels match, any format.
    if let Ok(mut configs) = device.supported_output_configs()
        && let Some(c) = configs.find(|c| {
            c.channels() == desired_channels
                && c.min_sample_rate() <= desired_rate
                && desired_rate <= c.max_sample_rate()
        })
    {
        return Ok(c.with_sample_rate(desired_rate));
    }
    // (4) Rate match, any channels/format.
    if let Ok(mut configs) = device.supported_output_configs()
        && let Some(c) = configs.find(|c| {
            c.min_sample_rate() <= desired_rate && desired_rate <= c.max_sample_rate()
        })
    {
        return Ok(c.with_sample_rate(desired_rate));
    }
    // (5) Last resort: device default.
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
///
/// `flush_gen` is a generation counter shared with [`CpalSink::flush`]. When
/// the worker bumps it, the callback drains-and-discards the entire ring
/// buffer on its next invocation before producing output.
fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    consumer: HeapCons<f32>,
    volume: Arc<AtomicU32>,
    flush_gen: Arc<AtomicU32>,
) -> crate::Result<Stream>
where
    T: SizedSample + cpal::FromSample<f32> + Send + 'static,
{
    let mut consumer = consumer;
    let err_fn = |err: cpal::StreamError| tracing::error!("cpal stream error: {err}");

    // Last flush_gen value observed by the callback. When the worker-side
    // counter changes, the callback discards the entire buffer once. We
    // initialize to the current value so a flush that raced with stream
    // creation is still honored.
    let last_flush = AtomicU32::new(flush_gen.load(Ordering::Relaxed));

    let stream = device
        .build_output_stream::<T, _, _>(
            config,
            move |out: &mut [T], _info: &cpal::OutputCallbackInfo| {
                // Flush check: if the worker bumped the generation, discard
                // everything currently in the ring buffer (whether produced
                // before or racing with this callback) before producing
                // output. We `clear()` then advance our observed counter so
                // we only discard once per flush().
                let current_gen = flush_gen.load(Ordering::Acquire);
                let last_gen = last_flush.load(Ordering::Relaxed);
                if current_gen != last_gen {
                    consumer.clear();
                    last_flush.store(current_gen, Ordering::Relaxed);
                    // Output silence for this period — the worker's
                    // post-seek samples haven't arrived yet (or only just
                    // started arriving after the clear). Filling silence
                    // avoids a partial-buffer glitch.
                    for slot in out.iter_mut() {
                        *slot = T::EQUILIBRIUM;
                    }
                    return;
                }

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
