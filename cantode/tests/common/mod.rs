//! Shared test utilities — used by every integration test in `cantode`.
//!
//! Two groups:
//!
//! - WAV generation + [`require_audio_device`] — for the real-device
//!   tests (`tests/playback.rs`, ...).
//! - The device-free harness pieces — [`CaptureSink`] /
//!   [`capture_factory`] (records everything the worker pushes, no
//!   audio device), the [`wait_until`] / [`wait_for_ended`] /
//!   [`wait_for_quiet`] detectors, and [`reference_decode`] (the
//!   bit-exact oracle). These power `tests/network_source.rs` and
//!   `tests/remote_source.rs`.
//!
//! Tests in this crate run against the **real** cpal audio device; there
//! is no NullSink or other fake output backend. If the host has no output
//! device, [`require_audio_device`] panics with a clear message — we
//! consider "the test environment has no audio device" a real failure, not
//! something to silently skip. CI runners without a physical sound card
//! should install a virtual loopback/null device (e.g. `pulseaudio`'s
//! `module-null-sink`, macOS `BlackHole`, or Windows's WASAPI default
//! which always has a software endpoint).

use std::f32::consts::TAU;

/// Mono 16-bit 44.1 kHz — the sample rate every WAV helper here and
/// the device-free harnesses below assume.
#[allow(dead_code)]
pub const RATE: u32 = 44_100;

// ============================================================================
// Device-free capture sink — records everything the worker pushes
// ============================================================================

/// One recorded sink call (the sample payload itself lives in the
/// shared `samples` vec, indexed by `Write.offset`).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum CapEvent {
    Start { channels: u16, sample_rate: u32 },
    Write { offset: usize, len: usize },
    Flush,
    Pause,
    Resume,
    Stop,
    SetVolume,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct CaptureState {
    pub samples: Vec<f32>,
    pub events: Vec<CapEvent>,
}

/// A [`cantode::AudioSink`] that records calls and samples, optionally
/// pacing writes to real time (device emulation with a 0.5 s buffer).
#[allow(dead_code)]
pub struct CaptureSink {
    state: std::sync::Arc<std::sync::Mutex<CaptureState>>,
    pace_realtime: bool,
    sample_rate: u32,
    channels: u16,
    started_at: Option<std::time::Instant>,
    written: usize,
}

impl CaptureSink {
    /// Build a sink recording into `state` (shared so a factory-made
    /// sink and the test assertions see the same log).
    #[allow(dead_code)]
    pub fn shared(
        state: &std::sync::Arc<std::sync::Mutex<CaptureState>>,
        pace_realtime: bool,
    ) -> Self {
        Self {
            state: std::sync::Arc::clone(state),
            pace_realtime,
            sample_rate: RATE,
            channels: 1,
            started_at: None,
            written: 0,
        }
    }
}

impl cantode::AudioSink for CaptureSink {
    fn start(&mut self, fmt: cantode::AudioFormat) -> cantode::Result<cantode::AudioFormat> {
        self.sample_rate = fmt.sample_rate;
        self.channels = fmt.channels;
        self.started_at = Some(std::time::Instant::now());
        self.state.lock().unwrap().events.push(CapEvent::Start {
            channels: fmt.channels,
            sample_rate: fmt.sample_rate,
        });
        // Echo the format back: the worker's channel conversion stays on
        // the passthrough path, so captured samples are exactly the
        // decoder output.
        Ok(fmt)
    }

    fn stop(&mut self) -> cantode::Result<()> {
        self.state.lock().unwrap().events.push(CapEvent::Stop);
        Ok(())
    }

    fn write(&mut self, frames: &[f32]) -> cantode::Result<()> {
        if self.pace_realtime {
            // Device-pace emulation with a 0.5s buffer: a real sink accepts
            // writes until its buffer is full, then blocks while the device
            // drains at 1x. Cumulative written audio may lead wall-clock by
            // LEAD at most; blocking here blocks the worker — exactly how a
            // real sink applies backpressure.
            const LEAD: std::time::Duration = std::time::Duration::from_millis(500);
            let frames_total = (self.written + frames.len()) / self.channels.max(1) as usize;
            let audio_time =
                std::time::Duration::from_secs_f64(frames_total as f64 / self.sample_rate as f64);
            if let Some(t0) = self.started_at {
                let budget = t0.elapsed() + LEAD;
                if audio_time > budget {
                    std::thread::sleep(audio_time - budget);
                }
            }
        }
        let mut st = self.state.lock().unwrap();
        let offset = st.samples.len();
        st.samples.extend_from_slice(frames);
        st.events.push(CapEvent::Write {
            offset,
            len: frames.len(),
        });
        self.written += frames.len();
        Ok(())
    }

    fn flush(&mut self) -> cantode::Result<()> {
        self.state.lock().unwrap().events.push(CapEvent::Flush);
        Ok(())
    }

    fn pause(&mut self) -> cantode::Result<()> {
        self.state.lock().unwrap().events.push(CapEvent::Pause);
        Ok(())
    }

    fn resume(&mut self) -> cantode::Result<()> {
        self.state.lock().unwrap().events.push(CapEvent::Resume);
        Ok(())
    }

    fn set_volume(&mut self, _vol: f32) -> cantode::Result<()> {
        self.state.lock().unwrap().events.push(CapEvent::SetVolume);
        Ok(())
    }

    fn latency(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}

/// Build a `(shared capture state, factory)` pair for injecting a
/// [`CaptureSink`] via `PlayerConfig::audio_sink_factory`.
#[allow(dead_code)]
pub fn capture_factory(
    pace: bool,
) -> (
    std::sync::Arc<std::sync::Mutex<CaptureState>>,
    cantode::AudioSinkFactory,
) {
    let state = std::sync::Arc::new(std::sync::Mutex::new(CaptureState::default()));
    let sink_state = std::sync::Arc::clone(&state);
    let factory: cantode::AudioSinkFactory =
        std::sync::Arc::new(move || Ok(Box::new(CaptureSink::shared(&sink_state, pace))));
    (state, factory)
}

// ============================================================================
// Waiting helpers
// ============================================================================

/// Poll `pred` every 10 ms until it holds or `timeout` elapses.
#[allow(dead_code)]
pub fn wait_until(timeout: std::time::Duration, mut pred: impl FnMut() -> bool) -> bool {
    let end = std::time::Instant::now() + timeout;
    loop {
        if pred() {
            return true;
        }
        if std::time::Instant::now() >= end {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Drain the event receiver until `Ended` arrives (or timeout).
#[allow(dead_code)]
pub fn wait_for_ended(
    rx: &std::sync::mpsc::Receiver<cantode::PlayerEvent>,
    timeout: std::time::Duration,
) -> bool {
    use std::sync::mpsc::RecvTimeoutError;
    let end = std::time::Instant::now() + timeout;
    loop {
        let remaining = end.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match rx.recv_timeout(remaining.min(std::time::Duration::from_millis(100))) {
            Ok(cantode::PlayerEvent::Ended) => return true,
            Ok(_) => continue,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

/// Wait until `sample` stops changing for `quiet` — the detector for
/// "playback froze". A stall/error does not freeze the position
/// instantly: decode keeps advancing through already-buffered bytes for
/// a while first, so waiting for quiet is the honest signal.
#[allow(dead_code)]
pub fn wait_for_quiet<T: PartialEq + Copy>(
    deadline: std::time::Duration,
    quiet: std::time::Duration,
    mut sample: impl FnMut() -> T,
) -> Option<T> {
    let end = std::time::Instant::now() + deadline;
    let mut last = sample();
    let mut last_change = std::time::Instant::now();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let now = sample();
        if now != last {
            last = now;
            last_change = std::time::Instant::now();
        } else if last_change.elapsed() >= quiet {
            return Some(last);
        }
        if std::time::Instant::now() >= end {
            return None;
        }
    }
}

// ============================================================================
// Decode oracle
// ============================================================================

/// Decode the same bytes from RAM — the bit-exact reference every
/// output-correctness assertion compares against. Symphonia is
/// deterministic, so the reference decode is a valid oracle.
#[allow(dead_code)]
pub fn reference_decode(wav_bytes: &[u8]) -> Vec<f32> {
    use cantode::DecoderFactory;
    let factory = cantode::SymphoniaDecoderFactory::new();
    let mut dec = factory
        .open(Box::new(cantode::MemoryAudioSource::new(
            wav_bytes.to_vec(),
        )))
        .expect("reference open");
    let mut out = Vec::new();
    while let Some(frame) = dec.next_frame().expect("reference decode") {
        out.extend_from_slice(&frame.data);
    }
    out
}

/// Parameters for [`make_sine_wav`].
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct WavSpec {
    pub sample_rate: u32,
    pub channels: u16,
    /// Duration in seconds.
    pub seconds: f32,
    /// Sine frequency in Hz.
    pub freq: f32,
}

impl Default for WavSpec {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            channels: 1,
            seconds: 1.0,
            freq: 440.0,
        }
    }
}

/// Build a 16-bit PCM WAV byte stream of a sine wave per `spec`.
pub fn make_sine_wav(spec: WavSpec) -> Vec<u8> {
    let total_frames = (spec.sample_rate as f32 * spec.seconds).round() as usize;
    let data_bytes = total_frames * spec.channels as usize * 2; // 16-bit
    let mut out = Vec::with_capacity(44 + data_bytes);

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&spec.channels.to_le_bytes());
    out.extend_from_slice(&spec.sample_rate.to_le_bytes());
    let byte_rate = spec.sample_rate * spec.channels as u32 * 2;
    out.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = spec.channels * 2;
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());

    for fr in 0..total_frames {
        let t = fr as f32 / spec.sample_rate as f32;
        let amp = (t * spec.freq * TAU).sin();
        let scaled = (amp * i16::MAX as f32) as i16;
        // Same sample on every channel (mono content replicated).
        let bytes = scaled.to_le_bytes();
        for _ in 0..spec.channels {
            out.extend_from_slice(&bytes);
        }
    }
    out
}

/// Require that an output audio device exists on the host. Panics with a
/// clear message if not. Tests that touch the `Player` API call this at
/// the top so the failure is easy to diagnose.
#[allow(dead_code)] // not every test crate uses it
pub fn require_audio_device() {
    // We don't depend on cpal directly in tests; we infer device presence
    // by attempting a PlayerContext + Player load against a 1-sample sine.
    // If `load` fails with `NoOutputDevice`, the environment is missing
    // audio — fail loud.
    use cantode::{AudioSource, CantodeError, MemoryAudioSource, PlayerContext};
    let cx = match PlayerContext::new() {
        Ok(cx) => cx,
        Err(e) => panic!(
            "PlayerContext::new() failed: {e:?}. \
             Tests require a working cpal host."
        ),
    };
    let player = cantode::Player::new(&cx).expect("Player::new failed");
    // 1 sample of silence at 8 kHz mono — opens the device, doesn't play
    // anything audible.
    let mut bytes = make_sine_wav(WavSpec {
        sample_rate: 8_000,
        channels: 1,
        seconds: 0.001,
        freq: 1.0,
    });
    bytes.truncate(44 + 16); // just enough payload for a valid tiny WAV
    let src: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(bytes));
    match player.load(src) {
        Ok(_) => {
            // Quiet any possible output from this probe.
            let _ = player.set_volume(0.0);
            let _ = player.play();
            let _ = player.stop();
        }
        Err(CantodeError::NoOutputDevice) => {
            panic!(
                "No audio output device found on this host. \
                 cantode tests run against the real cpal device; install a \
                 virtual loopback/null sink (PulseAudio module-null-sink, \
                 macOS BlackHole, etc.) and re-run."
            );
        }
        Err(e) => {
            // Other errors (decode failures on a 1-sample WAV, etc.) are
            // not what this probe is about — surface them too.
            panic!("audio device probe failed unexpectedly: {e:?}");
        }
    }
}
