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
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

use cpal::{
    Sample, SizedSample, Stream, StreamConfig, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

use crate::{CantodeError, decoder::AudioFormat, output::AudioSink};

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

/// The realtime output clock shared between the worker-side sink and the
/// cpal callback: the callback counts what it actually plays, and the
/// worker anchors that count to media time. [`CpalSink::output_position`]
/// then reads as "media time of the sample currently being mixed" instead
/// of the decode frontier (which leads the audio by the whole ring buffer).
///
/// - The **callback** is the only writer of `played` (one relaxed
///   `fetch_add` per drain — RT-safe, no locks). Samples popped from the
///   ring count; silence written on underflow does not (media time must
///   freeze while the listener hears silence).
/// - The **worker** publishes the anchor `(head_ts, head_played)` under a
///   seqlock on empty-ring writes (rare: session start, post-flush/seek,
///   post-underflow refill). An anchor means: "the sample at media time
///   `head_ts` is the callback's next pop once `played == head_played`".
/// - Readers (never the callback) compute
///   `head_ts + (played − head_played) / samples_per_sec`.
///
/// The anchor is exact because of the ring's SPSC discipline: pops require
/// a non-empty ring, and the worker — having just observed an empty ring —
/// is the only party able to make it non-empty, so `played` cannot advance
/// between the observation and the anchor's `played` read.
struct OutputClock {
    /// Interleaved samples popped by the callback so far.
    played: AtomicU64,
    /// Seqlock version over the anchor pair (odd = write in flight).
    seq: AtomicU32,
    head_ts_nanos: AtomicU64,
    head_played: AtomicU64,
    /// Interleaved samples per second of the negotiated device format.
    samples_per_sec: u64,
}

impl OutputClock {
    fn new(samples_per_sec: u64) -> Self {
        Self {
            played: AtomicU64::new(0),
            seq: AtomicU32::new(0),
            head_ts_nanos: AtomicU64::new(0),
            head_played: AtomicU64::new(0),
            samples_per_sec: samples_per_sec.max(1),
        }
    }

    /// Callback side: account for `samples` interleaved samples actually
    /// popped from the ring.
    fn add_played(&self, samples: usize) {
        self.played.fetch_add(samples as u64, Ordering::Relaxed);
    }

    /// Worker side: publish a fresh anchor. Only valid when the ring was
    /// observed empty (see [`CpalSink::write`]).
    fn anchor(&self, ts: Duration) {
        let played = self.played.load(Ordering::Relaxed);
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1) | 1, Ordering::Release); // odd
        self.head_ts_nanos
            .store(ts.as_nanos() as u64, Ordering::Relaxed);
        self.head_played.store(played, Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(2), Ordering::Release); // even
    }

    /// Worker side: media time of the sample the callback is mixing right
    /// now (≈ what the listener hears, modulo the device's own short
    /// output buffer). Retries while an anchor write is in flight.
    fn position(&self) -> Duration {
        loop {
            let seq = self.seq.load(Ordering::Acquire);
            if seq & 1 == 1 {
                continue; // anchor write in flight; retry
            }
            let ts = self.head_ts_nanos.load(Ordering::Relaxed);
            let head_played = self.head_played.load(Ordering::Relaxed);
            let played = self.played.load(Ordering::Relaxed);
            if self.seq.load(Ordering::Acquire) != seq {
                continue; // anchor changed under us; retry
            }
            let extra = played.saturating_sub(head_played) as u128;
            let nanos = ts as u128 + extra * 1_000_000_000u128 / self.samples_per_sec as u128;
            return Duration::from_nanos(nanos.min(u64::MAX as u128) as u64);
        }
    }
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
    /// The realtime output clock, shared with the cpal callback once the
    /// stream is open (see [`OutputClock`]).
    clock: Option<Arc<OutputClock>>,
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
            clock: None,
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
        let actual = AudioFormat::new(stream_config.channels as u16, stream_config.sample_rate);

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
        let clock = Arc::new(OutputClock::new(
            actual.sample_rate as u64 * actual.channels as u64,
        ));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::I16 => build_stream::<i16>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::U16 => build_stream::<u16>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::F64 => build_stream::<f64>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::I32 => build_stream::<i32>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::U32 => build_stream::<u32>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::I8 => build_stream::<i8>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::U8 => build_stream::<u8>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::I64 => build_stream::<i64>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            cpal::SampleFormat::U64 => build_stream::<u64>(
                &device,
                &stream_config,
                consumer,
                volume,
                flush_gen,
                clock.clone(),
            )?,
            other => {
                return Err(CantodeError::StreamConfig(format!(
                    "unsupported sample format: {other:?}"
                )));
            }
        };

        self.stream = Some(stream);
        self.producer = Some(producer);
        self.clock = Some(clock);
        self.format = Some(actual);
        Ok(actual)
    }

    fn stop(&mut self) -> crate::Result<()> {
        self.stream = None;
        self.producer = None;
        self.clock = None;
        self.format = None;
        Ok(())
    }

    fn write(&mut self, frames: &[f32], start_ts: Duration) -> crate::Result<()> {
        let Some(producer) = self.producer.as_mut() else {
            // Pre-start writes are silently discarded; callers don't need
            // to special-case initial buffering.
            return Ok(());
        };

        // Output-clock anchor: when the ring is empty, the sample at
        // `start_ts` is the next one the callback will pop. Pops require a
        // non-empty ring and we are the only producer, so `played` cannot
        // advance between this observation and our push — the anchor is
        // exact. Anchors land exactly at the timestamp-discontinuity
        // points: session start, post-flush/seek, post-underflow refill.
        if producer.is_empty()
            && let Some(clock) = &self.clock
        {
            clock.anchor(start_ts);
        }

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

    fn output_position(&self) -> Option<Duration> {
        self.clock.as_ref().map(|c| c.position())
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
        && let Some(c) = configs
            .find(|c| c.min_sample_rate() <= desired_rate && desired_rate <= c.max_sample_rate())
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
/// Every sample actually popped is accounted on the [`OutputClock`] so
/// `output_position` tracks what the listener hears.
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
    clock: Arc<OutputClock>,
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
                    // avoids a partial-buffer glitch. Nothing was popped,
                    // so the output clock does not advance.
                    for slot in out.iter_mut() {
                        *slot = T::EQUILIBRIUM;
                    }
                    return;
                }

                let vol = load_vol(&volume);
                let played = drain_into(&mut consumer, out, vol);
                clock.add_played(played);
            },
            err_fn,
            None,
        )
        .map_err(|e| CantodeError::Sink(format!("build stream: {e}")))?;
    Ok(stream)
}

/// Drain the ring buffer into `out`, applying gain in f32 space, then
/// converting to the device sample type. Silence on underflow. Returns the
/// number of interleaved samples actually popped from the ring (the
/// underflow tail is silence and not counted).
fn drain_into<T: Sample + cpal::FromSample<f32>>(
    consumer: &mut HeapCons<f32>,
    out: &mut [T],
    vol: f32,
) -> usize {
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
    filled
}

#[cfg(test)]
mod tests {
    //! Unit tests for the [`OutputClock`] arithmetic — pure atomics, no
    //! audio device needed. The anchor/race discipline itself is exercised
    //! by the device-free player tests via the tracking stub sink.

    use super::*;

    /// 48 kHz stereo = 96 000 interleaved samples per second.
    const SPS: u64 = 96_000;

    #[test]
    fn clock_starts_at_zero_before_any_anchor() {
        let clock = OutputClock::new(SPS);
        assert_eq!(clock.position(), Duration::ZERO);
    }

    #[test]
    fn clock_reports_anchor_plus_played() {
        let clock = OutputClock::new(SPS);
        clock.anchor(Duration::from_secs(10));
        // Nothing popped yet: exactly at the anchor.
        assert_eq!(clock.position(), Duration::from_secs(10));
        // 96 000 samples = 1 s of stereo audio.
        clock.add_played(SPS as usize);
        assert_eq!(clock.position(), Duration::from_secs(11));
        clock.add_played(SPS as usize / 2);
        assert_eq!(clock.position(), Duration::from_millis(11_500));
    }

    #[test]
    fn clock_freezes_while_nothing_is_popped() {
        // Underflow silence must not advance media time.
        let clock = OutputClock::new(SPS);
        clock.anchor(Duration::from_secs(5));
        clock.add_played(SPS as usize);
        assert_eq!(clock.position(), Duration::from_secs(6));
        assert_eq!(clock.position(), Duration::from_secs(6));
        assert_eq!(clock.position(), Duration::from_secs(6));
    }

    #[test]
    fn clock_reanchor_resumes_from_the_new_timestamp() {
        // Post-seek: the flush discards the ring, the next empty-ring
        // write re-anchors at the new timestamp, and the played counter
        // continues monotonically from wherever it was.
        let clock = OutputClock::new(SPS);
        clock.anchor(Duration::from_secs(100));
        clock.add_played(SPS as usize);
        assert_eq!(clock.position(), Duration::from_secs(101));

        clock.anchor(Duration::from_secs(42));
        assert_eq!(clock.position(), Duration::from_secs(42));
        clock.add_played(SPS as usize / 4);
        assert_eq!(clock.position(), Duration::from_millis(42_250));
    }

    #[test]
    fn clock_clamps_before_the_first_pop_after_reanchor() {
        // A reader that observes the anchor before the callback accounts
        // the first pops must never report before the anchor.
        let clock = OutputClock::new(SPS);
        clock.anchor(Duration::from_secs(9));
        assert_eq!(clock.position(), Duration::from_secs(9));
    }
}
