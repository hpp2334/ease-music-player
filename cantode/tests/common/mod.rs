//! Shared test utilities — used by every integration test in `cantode`.
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
