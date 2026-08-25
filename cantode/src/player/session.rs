//! One decode session: everything that exists only while a source is
//! loaded and its sink is open.
//!
//! The session moves as a unit between the payload-carrying `Phase`
//! variants (`Paused` ↔ `Playing` ↔ `Buffering` → `Ended`); those
//! variant-to-variant moves destructure the session out rather than
//! dropping it. True session exit — stop, replace by a new load,
//! `Failed`, or worker teardown — happens in exactly one place:
//! [`Loaded::drop`]. Because the sink lives inside the session, "sink
//! open ⟹ decoder open" holds by construction instead of by discipline.
//!
//! The session owns the decode→render step (`pump`) and the
//! position observable: it writes the position when decoding, seeking,
//! and when the session dies. It emits no events — it returns decisions
//! ([`PumpOutcome`], seek results) and lets the worker do the emission.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::decoder::{AudioFormat, DecodedFrame, Decoder};
use crate::output::AudioSink;

use super::shared::SharedStatus;

pub(super) struct Loaded {
    decoder: Box<dyn Decoder>,
    sink: Box<dyn AudioSink>,
    /// Channel count of the currently-loaded source, captured at
    /// construction from `decoder.format()`. Zero means "no source
    /// loaded" / "no conversion needed yet".
    src_channels: u16,
    /// Channel count the device stream actually opened with (the format
    /// `sink.start` negotiated). When this differs from `src_channels`,
    /// `write_frame` runs each decoded frame through `remux_channels`
    /// before writing it to the sink. The mismatch is rare but real —
    /// see `CpalSink::start` / `pick_supported_config`.
    device_channels: u16,
    /// Reusable scratch buffer for the channel-conversion path. Empty when
    /// `src_channels == device_channels`.
    remix_buf: Vec<f32>,
    /// Latch: `PlayerEvent::Ended` has been emitted for this load already.
    ended: bool,
    /// The shared observables. The session writes `position` and resets
    /// both session-scoped observables on drop; `duration` is published
    /// by the worker after a successful open.
    shared: Arc<SharedStatus>,
}

/// What one [`Loaded::pump`] produced. The decode work happens under the
/// playing borrow in the worker; the worker acts on the emission
/// decisions after that borrow ends.
pub(super) enum PumpOutcome {
    /// A frame was decoded and written; `emit` says the position-event
    /// throttle elapsed for it.
    Frame { position: Duration, emit: bool },
    /// The decoder reached end of stream.
    EndOfStream,
    /// A (non-fatal) decode error; the frame was skipped.
    Skipped,
}

impl Loaded {
    /// Assemble a session from an opened decoder and a started sink.
    ///
    /// Captures the channel counts from `decoder.format()` and the
    /// negotiated `device_fmt` (the value `sink.start` returned), and
    /// logs when they differ. The sink must already be started.
    pub(super) fn new(
        decoder: Box<dyn Decoder>,
        sink: Box<dyn AudioSink>,
        device_fmt: AudioFormat,
        shared: Arc<SharedStatus>,
    ) -> Self {
        let src_channels = decoder.format().channels;
        let device_channels = device_fmt.channels;
        if src_channels != device_channels {
            tracing::info!(
                src = src_channels,
                device = device_channels,
                "device channel count differs from source; \
                 enabling channel conversion in worker"
            );
        }
        Self {
            decoder,
            sink,
            src_channels,
            device_channels,
            remix_buf: Vec::new(),
            ended: false,
            shared,
        }
    }

    /// Decode one frame and render it: store the position observable,
    /// write the samples (with channel conversion when the device
    /// insisted on a different layout), and decide whether the
    /// position-event throttle elapsed. `interval` is passed in — event
    /// cadence is the worker's policy, not the session's.
    pub(super) fn pump(&mut self, last_emit: &mut Instant, interval: Duration) -> PumpOutcome {
        match self.decoder.next_frame() {
            Ok(Some(frame)) => {
                self.shared.set_position(frame.timestamp);
                self.write_frame(&frame);
                let now = Instant::now();
                let emit = now.duration_since(*last_emit) >= interval;
                if emit {
                    *last_emit = now;
                }
                PumpOutcome::Frame {
                    position: frame.timestamp,
                    emit,
                }
            }
            Ok(None) => PumpOutcome::EndOfStream,
            Err(_e) => {
                // Non-fatal: skip. A future improvement is to surface
                // repeated decode failures via PlayerEvent::Error.
                tracing::debug!("decode error in pump; skipping frame");
                PumpOutcome::Skipped
            }
        }
    }

    /// Seek choreography in one place: decoder seek, flush the sink's
    /// buffered audio (it holds up to `buffer_secs` of pre-seek samples
    /// that would otherwise play out before the new position's audio
    /// arrives — without the flush the listener hears ~2s of stale audio
    /// mixed with the new position), clear the `ended` latch, and
    /// publish the new position. Returns the actual position for the
    /// caller's event emission.
    pub(super) fn seek(&mut self, target: Duration) -> crate::Result<Duration> {
        let actual = self.decoder.seek(target)?;
        let _ = self.sink.flush();
        self.ended = false;
        self.shared.set_position(actual);
        Ok(actual)
    }

    /// Linear gain on the sink. `1.0` is unity; `0.0` is silent.
    pub(super) fn set_volume(&mut self, vol: f32) {
        let _ = self.sink.set_volume(vol);
    }

    /// Gate output off without closing the stream (phase side effect on
    /// entering `Paused` / `Ended`).
    pub(super) fn pause(&mut self) {
        let _ = self.sink.pause();
    }

    /// Reopen output (phase side effect on entering `Playing`).
    pub(super) fn resume(&mut self) {
        let _ = self.sink.resume();
    }

    /// Whether the `Ended` latch is set for this load.
    pub(super) fn has_ended(&self) -> bool {
        self.ended
    }

    /// Set the `Ended` latch (the worker does this when it emits
    /// `PlayerEvent::Ended`, so the event fires exactly once per load).
    pub(super) fn mark_ended(&mut self) {
        self.ended = true;
    }

    /// Push one decoded frame to the sink, converting the channel layout
    /// when the device insisted on a count other than the source's. The
    /// common case (`src_channels == device_channels`) skips the scratch
    /// buffer entirely.
    fn write_frame(&mut self, frame: &DecodedFrame) {
        if self.src_channels != 0
            && self.device_channels != 0
            && self.src_channels != self.device_channels
        {
            let n_frames = frame.data.len() / self.src_channels as usize;
            let out_samples = n_frames * self.device_channels as usize;
            if self.remix_buf.len() < out_samples {
                self.remix_buf.resize(out_samples, 0.0);
            }
            let written = crate::output::remux_channels(
                &frame.data,
                self.src_channels,
                &mut self.remix_buf[..out_samples],
                self.device_channels,
            );
            let _ = self.sink.write(&self.remix_buf[..written]);
        } else {
            let _ = self.sink.write(&frame.data);
        }
    }
}

impl Drop for Loaded {
    fn drop(&mut self) {
        // Session teardown in exactly one place. Variant-to-variant moves
        // (which destructure the session out rather than dropping it)
        // never run this — only true session exit does: stop, replace by
        // a new load, `Failed`, or worker teardown.
        let _ = self.sink.stop();
        self.shared.reset_session_observables();
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the decode session — teardown, channel conversion,
    //! the pump, and the seek choreography — using the stub doubles from
    //! `super::super::stubs`.

    use std::time::Duration;

    use super::*;
    use crate::player::stubs::loaded_session;

    #[test]
    fn loaded_drop_is_the_single_teardown_point() {
        let (loaded, fx) = loaded_session(2, 2);
        fx.shared.set_position(Duration::from_secs(1));
        fx.shared.set_duration(Some(Duration::from_secs(1)));
        drop(loaded);
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position(), Duration::ZERO);
        assert_eq!(fx.shared.duration(), None);
    }

    #[test]
    fn write_frame_passes_through_when_channels_match() {
        let (mut loaded, fx) = loaded_session(2, 2);
        let frame = DecodedFrame {
            data: vec![0.1, 0.2, 0.3, 0.4],
            frames: 2,
            timestamp: Duration::ZERO,
        };
        loaded.write_frame(&frame);
        assert_eq!(*fx.log.samples(), frame.data);
        assert!(loaded.remix_buf.is_empty());
    }

    #[test]
    fn write_frame_remuxes_when_channels_differ() {
        let (mut loaded, fx) = loaded_session(2, 1);
        let frame = DecodedFrame {
            // (0.2, 0.6) → 0.4 ; (1.0, -1.0) → 0.0
            data: vec![0.2, 0.6, 1.0, -1.0],
            frames: 2,
            timestamp: Duration::ZERO,
        };
        loaded.write_frame(&frame);
        let samples = fx.log.samples().clone();
        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.4).abs() < 1e-6);
        assert!(samples[1].abs() < 1e-6);
    }

    #[test]
    fn seek_flushes_publishes_position_and_clears_the_latch() {
        let (mut loaded, fx) = loaded_session(2, 2);
        loaded.mark_ended();

        let actual = loaded.seek(Duration::from_secs(5)).unwrap();

        assert_eq!(actual, Duration::from_secs(5));
        assert!(fx.log.recorded("flush"));
        assert!(!loaded.has_ended());
        assert_eq!(fx.shared.position(), Duration::from_secs(5));
    }

    #[test]
    fn pump_end_of_stream_touches_nothing() {
        // The stub decoder yields no frames, so a pump is an EOF.
        let (mut loaded, fx) = loaded_session(2, 2);
        let mut last = Instant::now();
        assert!(matches!(
            loaded.pump(&mut last, Duration::from_millis(100)),
            PumpOutcome::EndOfStream
        ));
        assert!(fx.log.samples().is_empty());
        assert_eq!(fx.shared.position(), Duration::ZERO);
    }
}
