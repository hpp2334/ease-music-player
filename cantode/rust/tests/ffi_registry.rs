//! Registry round-trip for the `ffi` surface (feature-gated — the module
//! doesn't exist without it). Drives the plain-Rust registry API the
//! backend embedder uses; the JNI exports themselves need a live JNIEnv
//! and are exercised on-device by the app.

#![cfg(feature = "ffi")]

use std::sync::Arc;

use cantode::{Player, PlayerContext, PlayerState, ffi, source::MemoryAudioSource};

mod common;

#[test]
fn player_registry_weak_reflection_tracks_owner() {
    common::require_audio_device();

    let cx = PlayerContext::new().unwrap();
    let player = Arc::new(Player::new(&cx).unwrap());
    ffi::register_player(42, &player);
    assert!(ffi::player(42).is_some());

    // Weak: dropping the owner deadens the handle (no unregister call).
    drop(player);
    assert!(ffi::player(42).is_none());
}

#[test]
fn source_registry_consumes_tokens() {
    let src = MemoryAudioSource::new(common::make_sine_wav(common::WavSpec {
        seconds: 0.1,
        ..Default::default()
    }));
    let token = ffi::register_source(Box::new(src));
    assert!(ffi::take_source(token).is_some());
    // Taking is consuming — a replayed token is a safe miss.
    assert!(ffi::take_source(token).is_none());
}

#[test]
fn load_and_play_via_registry_reaches_playing() {
    common::require_audio_device();

    let cx = PlayerContext::new().unwrap();
    let player = Arc::new(Player::new(&cx).unwrap());
    player.set_volume(0.0).unwrap();
    ffi::register_player(7, &player);

    let src = MemoryAudioSource::new(common::make_sine_wav(common::WavSpec {
        seconds: 1.0,
        ..Default::default()
    }));
    let token = ffi::register_source(Box::new(src));

    // Same path `CantodeNative.loadAndPlay` drives: registry player +
    // consumed source, straight into Playing.
    let p = ffi::player(7).unwrap();
    let source = ffi::take_source(token).unwrap();
    p.load_and_play(source).unwrap();
    assert_eq!(p.state(), PlayerState::Playing);

    // `stop` posts a command — the worker applies it asynchronously, so
    // spin like `playback::stop_returns_to_idle` does.
    p.stop().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while p.state() != PlayerState::Idle {
        assert!(
            std::time::Instant::now() < deadline,
            "player did not reach Idle within 3s; state = {:?}",
            p.state()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
