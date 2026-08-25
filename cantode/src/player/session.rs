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

use std::sync::Arc;
use std::time::Duration;

use crate::decoder::DecodedFrame;

use super::shared::SharedStatus;

pub(super) struct Loaded {
    pub(super) decoder: Box<dyn crate::decoder::Decoder>,
    pub(super) sink: Box<dyn crate::output::AudioSink>,
    /// Channel count of the currently-loaded source, captured in `do_load`.
    /// Zero means "no source loaded" / "no conversion needed yet".
    pub(super) src_channels: u16,
    /// Channel count the device stream actually opened with, captured in
    /// `do_load` from the format returned by `sink.start`. When this
    /// differs from `src_channels`, `write_frame` runs each decoded frame
    /// through `remux_channels` before writing it to the sink. The
    /// mismatch is rare but real — see `CpalSink::start` /
    /// `pick_supported_config`.
    pub(super) device_channels: u16,
    /// Reusable scratch buffer for the channel-conversion path. Empty when
    /// `src_channels == device_channels`.
    pub(super) remix_buf: Vec<f32>,
    /// Latch: `PlayerEvent::Ended` has been emitted for this load already.
    pub(super) ended: bool,
    /// Clone of the worker's shared status, so the session-scoped
    /// observables (position, duration) reset exactly when the session
    /// dies — wherever that happens from.
    pub(super) shared: Arc<SharedStatus>,
}

impl Loaded {
    /// Push one decoded frame to the sink, converting the channel layout
    /// when the device insisted on a count other than the source's. The
    /// common case (`src_channels == device_channels`) skips the scratch
    /// buffer entirely.
    pub(super) fn write_frame(&mut self, frame: &DecodedFrame) {
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
        self.shared.position.store(Duration::ZERO);
        *self.shared.duration.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the decode session — teardown, channel conversion —
    //! using the stub doubles from `super::super::stubs`.

    use std::time::Duration;

    use super::*;
    use crate::player::stubs::loaded_session;

    #[test]
    fn loaded_drop_is_the_single_teardown_point() {
        let (loaded, fx) = loaded_session(2, 2);
        fx.shared.position.store(Duration::from_secs(1));
        *fx.shared.duration.lock().unwrap() = Some(Duration::from_secs(1));
        drop(loaded);
        assert!(fx.log.recorded("stop"));
        assert_eq!(fx.shared.position.load(), Duration::ZERO);
        assert_eq!(*fx.shared.duration.lock().unwrap(), None);
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
        assert_eq!(*fx.log.samples.lock().unwrap(), frame.data);
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
        let samples = fx.log.samples.lock().unwrap().clone();
        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.4).abs() < 1e-6);
        assert!(samples[1].abs() < 1e-6);
    }
}
