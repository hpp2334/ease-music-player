//! Test doubles shared by the unit tests across the `player` module
//! tree — a stub decoder/factory/sink plus the `loaded_session` fixture
//! builder — so the unit tests need no audio device. End-to-end behavior
//! is covered by `tests/` against the real cpal host.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    AudioSource, CantodeError, Metadata,
    decoder::{AudioFormat, DecodedFrame, Decoder, DecoderFactory},
    output::AudioSink,
};

use super::session::Loaded;
use super::shared::SharedStatus;

/// Decoder stub: reports a fixed format, yields no frames. Private to
/// this module — only `loaded_session` constructs it.
struct StubDecoder {
    pub(super) fmt: AudioFormat,
}

impl Decoder for StubDecoder {
    fn next_frame(&mut self) -> crate::Result<Option<DecodedFrame>> {
        Ok(None)
    }
    fn seek(&mut self, target: Duration) -> crate::Result<Duration> {
        Ok(target)
    }
    fn format(&self) -> AudioFormat {
        self.fmt
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
    pub(super) samples: Arc<Mutex<Vec<f32>>>,
}

impl SinkLog {
    fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_string());
    }
    pub(super) fn recorded(&self, call: &str) -> bool {
        self.calls.lock().unwrap().iter().any(|c| c == call)
    }
}

/// Sink stub: passes every written sample into a shared `SinkLog`.
/// Private to this module — only `loaded_session` constructs it.
struct StubSink {
    log: SinkLog,
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
    fn write(&mut self, frames: &[f32]) -> crate::Result<()> {
        self.log.samples.lock().unwrap().extend_from_slice(frames);
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
    fn latency(&self) -> Duration {
        Duration::ZERO
    }
}

/// A `Loaded` session backed by stubs, plus the handles to observe it.
pub(super) struct Fixture {
    pub(super) log: SinkLog,
    pub(super) shared: Arc<SharedStatus>,
}

/// Build a `Loaded` session backed by stubs, plus the handles to
/// observe it (sink call log + shared observables).
pub(super) fn loaded_session(src: u16, device: u16) -> (Loaded, Fixture) {
    let log = SinkLog::default();
    let shared = Arc::new(SharedStatus::new());
    let loaded = Loaded {
        decoder: Box::new(StubDecoder {
            fmt: AudioFormat::new(src, 48_000),
        }),
        sink: Box::new(StubSink { log: log.clone() }),
        src_channels: src,
        device_channels: device,
        remix_buf: Vec::new(),
        ended: false,
        shared: Arc::clone(&shared),
    };
    (loaded, Fixture { log, shared })
}
