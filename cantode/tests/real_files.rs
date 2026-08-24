//! Real-audio-file integration tests.
//!
//! Unlike the other test files which synthesize a sine WAV in memory, this
//! file exercises the decoder + player pipeline against **real** audio
//! bitstreams checked into `tests/samples/`:
//!
//! - `sample-3s.mp3`, `sample-9s.mp3`, `sample-15s.mp3` — real MP3s (CBR/VBR,
//!   ID3v2-tagged) from samplelib.com.
//! - `sample-9s.wav` — real stereo 16-bit PCM WAV from samplelib.com.
//! - `piano2.wav` — real 6s mono piano WAV (kozco.com reference sample).
//!
//! The samples are gitignored (network-fetched on demand). The probe
//! portion (duration/tags/etc.) runs unconditionally if a sample is
//! present; the full-decode portion through the cpal device runs only when
//! the host has an output device — otherwise it panics via
//! [`common::require_audio_device`].
//!
//! To keep the suite fast, the decode portion plays each file for ~500ms
//! (enough to confirm the device is being driven) and then stops; it does
//! not play every file to EOF.

#![allow(clippy::needless_pass_by_value)]

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use cantode::{
    AudioSource, MemoryAudioSource, Player, PlayerConfig, PlayerContext, PlayerEvent, PlayerState,
};

mod common;

/// Directory containing the real-audio samples.
fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
}

/// Read a sample file, or skip the test if it's missing.
fn load_sample(name: &str) -> Option<Vec<u8>> {
    let path = samples_dir().join(name);
    match fs::read(&path) {
        Ok(bytes) => {
            // Sanity-check: refuse to "test" against an HTML error page
            // mistakenly saved as audio.
            if bytes.starts_with(b"<!DOCTYPE") || bytes.starts_with(b"<html") {
                eprintln!(
                    "[skip] {name}: file looks like HTML (probably a failed download); \
                     re-fetch the sample"
                );
                return None;
            }
            Some(bytes)
        }
        Err(e) => {
            eprintln!("[skip] {name}: not present ({e}); skipping real-file test");
            None
        }
    }
}

/// One real-audio test case: expected duration + minimum sample rate we
/// consider reasonable for the file.
struct Case {
    file: &'static str,
    min_duration_ms: u64,
    max_duration_ms: u64,
    min_sample_rate: u32,
}

/// Run `probe_metadata` + a brief cpal-device playback against one real
/// file, asserting that the reported duration/sample-rate are sane and
/// that the cpal sink actually drives the player's position forward.
fn exercise_file(case: Case) {
    let bytes = match load_sample(case.file) {
        Some(b) => b,
        None => return, // sample absent or HTML — skip silently.
    };
    println!("[test] {} — {} bytes", case.file, bytes.len());

    // --- probe_metadata (does not touch the device) ---
    let cx = PlayerContext::new().expect("context construction failed");
    let probe_src: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(bytes.clone()));
    let meta = cantode::probe_metadata(&cx, probe_src)
        .unwrap_or_else(|e| panic!("probe_metadata({}) failed: {e:?}", case.file));

    println!(
        "[test]   probe: format = {} Hz / {} ch, duration = {:?}, total_samples = {:?}, \
         tags = {}, cover_art = {}",
        meta.format.sample_rate,
        meta.format.channels,
        meta.duration,
        meta.total_samples,
        meta.tags.len(),
        meta.cover_art.is_some()
    );

    assert!(
        meta.format.sample_rate >= case.min_sample_rate,
        "{}: sample_rate {} below minimum {}",
        case.file,
        meta.format.sample_rate,
        case.min_sample_rate
    );
    assert!(
        meta.format.channels >= 1,
        "{}: no channels reported",
        case.file
    );
    let dur = meta.duration.unwrap_or_else(|| {
        panic!(
            "{}: duration was None; probe failed to extract it",
            case.file
        )
    });
    let ms = dur.as_millis() as u64;
    assert!(
        ms >= case.min_duration_ms && ms <= case.max_duration_ms,
        "{}: duration {}ms outside expected range [{}, {}]",
        case.file,
        ms,
        case.min_duration_ms,
        case.max_duration_ms
    );

    // --- brief cpal-device playback ---
    // Don't even attempt the device path if the host has no output device:
    // `require_audio_device` will panic loudly so CI failures are obvious.
    common::require_audio_device();

    let cx2 = PlayerContext::new().expect("context construction failed");
    let event_sink = Arc::new(cantode::ChannelEventSink::new(1024));
    let rx = event_sink.subscribe();
    let player = Player::with_config(
        &cx2,
        PlayerConfig {
            event_sink: Some(event_sink),
        },
    )
    .expect("player construction failed");
    // Silent: we don't want test runs to make noise.
    player.set_volume(0.0).unwrap();

    let src: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(bytes));
    let load_meta = player
        .load(src)
        .unwrap_or_else(|e| panic!("Player::load({}) failed: {e:?}", case.file));
    assert_eq!(
        load_meta.duration, meta.duration,
        "{}: load duration disagrees with probe duration",
        case.file
    );
    assert_eq!(
        player.state(),
        PlayerState::Paused,
        "{}: player should be Paused after load",
        case.file
    );

    player.play().expect("play failed");

    // Play for ~500ms, confirming the position advances. This proves the
    // cpal callback is draining the ring buffer (otherwise the worker's
    // reported position wouldn't track wall-clock). We don't need to wait
    // for Ended — that's covered for a short source in playback.rs.
    let target = Duration::from_millis(400);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut saw_playing = false;
    while std::time::Instant::now() < deadline {
        if let Ok(PlayerEvent::StateChanged(PlayerState::Playing)) =
            rx.recv_timeout(Duration::from_millis(50))
        {
            saw_playing = true;
        }
        if player.position() >= target {
            break;
        }
    }
    assert!(saw_playing, "{}: never observed Playing state", case.file);
    assert!(
        player.position() >= target,
        "{}: position only reached {:?} in 3s; cpal callback may not be draining",
        case.file,
        player.position()
    );
    println!(
        "[test]   decode: Playing, position advanced to {:?} (target {:?})",
        player.position(),
        target
    );

    player.stop().unwrap();
}

#[test]
fn real_samplelib_mp3_3s() {
    exercise_file(Case {
        file: "sample-3s.mp3",
        min_duration_ms: 2_500,
        max_duration_ms: 4_000,
        min_sample_rate: 44_100,
    });
}

#[test]
fn real_samplelib_mp3_9s() {
    exercise_file(Case {
        file: "sample-9s.mp3",
        min_duration_ms: 8_000,
        max_duration_ms: 10_000,
        min_sample_rate: 44_100,
    });
}

#[test]
fn real_samplelib_mp3_15s() {
    // samplelib labels this file "15s" but it's actually ~19s (VBR).
    exercise_file(Case {
        file: "sample-15s.mp3",
        min_duration_ms: 18_000,
        max_duration_ms: 20_500,
        min_sample_rate: 44_100,
    });
}

#[test]
fn real_samplelib_wav_9s() {
    exercise_file(Case {
        file: "sample-9s.wav",
        min_duration_ms: 8_000,
        max_duration_ms: 10_000,
        min_sample_rate: 44_100,
    });
}

#[test]
fn real_kozco_piano_wav() {
    // kozco's piano2.wav is a 48 kHz stereo file that decodes to ~6.3s.
    exercise_file(Case {
        file: "piano2.wav",
        min_duration_ms: 5_500,
        max_duration_ms: 7_500,
        min_sample_rate: 22_050,
    });
}
