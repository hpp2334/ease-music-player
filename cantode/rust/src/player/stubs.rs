//! Test doubles shared by the unit tests across the `player` module
//! tree — a stub decoder/factory/sink plus the `loaded_session` fixture
//! builder — so the unit tests need no audio device. End-to-end behavior
//! is covered by `tests/` against the real cpal host.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    AudioSource, BufferedRange, CantodeError, Metadata,
    decoder::{AudioFormat, DecodedFrame, Decoder, DecoderFactory},
    output::AudioSink,
};

use super::session::Loaded;
use super::shared::SharedStatus;

/// Decoder stub: reports a fixed format, yields no frames. Private to
/// the `player` module tree (the session tests construct it directly).
pub(super) struct StubDecoder {
    pub(super) fmt: AudioFormat,
    /// When set, `next_frame` yields this error once (for the pump's
    /// outcome mapping tests) and returns to EOF afterwards.
    pub(super) fail_once: Option<crate::CantodeError>,
    /// What `buffered_range` reports (for the buffered-mirror tests);
    /// `None` = non-buffering source, the common default.
    pub(super) buffered: Option<BufferedRange>,
}

impl Decoder for StubDecoder {
    fn next_frame(&mut self) -> crate::Result<Option<DecodedFrame>> {
        if let Some(e) = self.fail_once.take() {
            return Err(e);
        }
        Ok(None)
    }
    fn seek(&mut self, target: Duration) -> crate::Result<Duration> {
        Ok(target)
    }
    fn format(&self) -> AudioFormat {
        self.fmt
    }
    fn buffered_range(&self) -> Option<BufferedRange> {
        self.buffered
    }
    fn metadata(&self) -> &Metadata {
        // The worker never reads metadata from the stub (do_load is
        // not exercised here); a static default is enough.
        static META: Metadata = Metadata {
            format: AudioFormat::new(0, 0),
            duration: None,
            total_samples: None,
            tags: Vec::new(),
            cover_art: None,
        };
        &META
    }
}

/// Decoder stub that yields exactly one frame (then EOF) — for tests
/// that need a real decode frontier (`decoded_through`).
pub(super) struct FrameDecoder {
    pub(super) frame: DecodedFrame,
    pub(super) yielded: bool,
}

impl Decoder for FrameDecoder {
    fn next_frame(&mut self) -> crate::Result<Option<DecodedFrame>> {
        if self.yielded {
            return Ok(None);
        }
        self.yielded = true;
        Ok(Some(self.frame.clone()))
    }
    fn seek(&mut self, target: Duration) -> crate::Result<Duration> {
        Ok(target)
    }
    fn format(&self) -> AudioFormat {
        // 2 ch @ 48 kHz — matches the stub sink's negotiated format.
        AudioFormat::new(2, 48_000)
    }
    fn metadata(&self) -> &Metadata {
        static META: Metadata = Metadata {
            format: AudioFormat::new(0, 0),
            duration: None,
            total_samples: None,
            tags: Vec::new(),
            cover_art: None,
        };
        &META
    }
}

/// Decoder factory stub — present so a `Worker` can be constructed;
/// these tests never call `do_load`.
pub(super) struct StubFactory;

impl DecoderFactory for StubFactory {
    fn open(&self, _source: Box<dyn AudioSource>) -> crate::Result<Box<dyn Decoder>> {
        Err(CantodeError::Internal("stub factory".into()))
    }
}

/// Sink stub: records every call name and every written sample.
#[derive(Clone, Default)]
pub(super) struct SinkLog {
    calls: Arc<Mutex<Vec<String>>>,
    samples: Arc<Mutex<Vec<f32>>>,
}

impl SinkLog {
    fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_string());
    }
    pub(super) fn recorded(&self, call: &str) -> bool {
        self.calls.lock().unwrap().iter().any(|c| c == call)
    }
    /// The samples written so far.
    pub(super) fn samples(&self) -> std::sync::MutexGuard<'_, Vec<f32>> {
        self.samples.lock().unwrap()
    }
}

/// Sink stub: passes every written sample into a shared `SinkLog`, with
/// an optional simulated output clock. Private to this module — only
/// `loaded_session*` constructs it.
struct StubSink {
    log: SinkLog,
    /// Simulated output clock. `None` (the default) = untracked sink;
    /// `Some(d)` = "everything written through `d` has sounded" — each
    /// write advances it to that write's end (instant-play model).
    out_pos: Arc<Mutex<Option<Duration>>>,
}

impl AudioSink for StubSink {
    fn start(&mut self, _fmt: AudioFormat) -> crate::Result<AudioFormat> {
        self.log.record("start");
        Ok(AudioFormat::new(2, 48_000))
    }
    fn stop(&mut self) -> crate::Result<()> {
        self.log.record("stop");
        Ok(())
    }
    fn write(&mut self, frames: &[f32], start_ts: Duration) -> crate::Result<()> {
        self.log.samples.lock().unwrap().extend_from_slice(frames);
        let mut out_pos = self.out_pos.lock().unwrap();
        if let Some(pos) = out_pos.as_mut() {
            // Instant-play model: written audio has immediately sounded,
            // so the clock sits at the write's end (2 ch @ 48 kHz device).
            *pos = start_ts + Duration::from_secs_f64(frames.len() as f64 / 96_000.0);
        }
        Ok(())
    }
    fn flush(&mut self) -> crate::Result<()> {
        self.log.record("flush");
        Ok(())
    }
    fn pause(&mut self) -> crate::Result<()> {
        self.log.record("pause");
        Ok(())
    }
    fn resume(&mut self) -> crate::Result<()> {
        self.log.record("resume");
        Ok(())
    }
    fn set_volume(&mut self, _vol: f32) -> crate::Result<()> {
        Ok(())
    }
    fn output_position(&self) -> Option<Duration> {
        *self.out_pos.lock().unwrap()
    }
}

/// A `Loaded` session backed by stubs, plus the handles to observe it.
pub(super) struct Fixture {
    pub(super) log: SinkLog,
    pub(super) shared: Arc<SharedStatus>,
    /// The stub sink's simulated output clock (see [`StubSink`]).
    out_pos: Arc<Mutex<Option<Duration>>>,
}

impl Fixture {
    /// Enable output-position tracking on the stub sink (simulates a
    /// sink whose written audio has instantly sounded).
    pub(super) fn enable_output_tracking(&self) {
        *self.out_pos.lock().unwrap() = Some(Duration::ZERO);
    }

    /// Force the stub sink's reported output position (`None` disables
    /// tracking again).
    pub(super) fn set_output_position(&self, pos: Option<Duration>) {
        *self.out_pos.lock().unwrap() = pos;
    }
}

/// Build a `Loaded` session backed by stubs, plus the handles to
/// observe it (sink call log + shared observables). The stub decoder
/// reports `src` channels at 48 kHz; the "device" is told `device`
/// channels to exercise or skip the conversion path.
pub(super) fn loaded_session(src: u16, device: u16) -> (Loaded, Fixture) {
    loaded_session_with(
        StubDecoder {
            fmt: AudioFormat::new(src, 48_000),
            fail_once: None,
            buffered: None,
        },
        src,
        device,
    )
}

/// Like [`loaded_session`] but with an explicit decoder (e.g. one that
/// fails once) — for the pump outcome-mapping tests.
pub(super) fn loaded_session_with(
    decoder: impl Decoder + 'static,
    _src: u16,
    device: u16,
) -> (Loaded, Fixture) {
    let log = SinkLog::default();
    let shared = Arc::new(SharedStatus::new());
    let out_pos = Arc::new(Mutex::new(None));
    let sink = Box::new(StubSink {
        log: log.clone(),
        out_pos: Arc::clone(&out_pos),
    });
    let loaded = Loaded::new(
        Box::new(decoder),
        sink,
        AudioFormat::new(device, 48_000),
        Arc::clone(&shared),
    );
    (
        loaded,
        Fixture {
            log,
            shared,
            out_pos,
        },
    )
}
