//! End-to-end player-state tests.
//!
//! These drive the player's public surface and assert on observed
//! [`PlayerState`] values. Tests that call `Player::load` (i.e. open the
//! cpal output device) require an audio device on the host — see
//! [`common::require_audio_device`].

use std::time::Duration;

use cantode::{MemoryAudioSource, Player, PlayerContext, PlayerState};

mod common;

/// `Player` starts in `Idle` and ends in `Idle` after a stop. Doesn't open
/// the device, so no `require_audio_device` needed.
#[test]
fn player_starts_idle() {
    let cx = PlayerContext::new().unwrap();
    let player = Player::new(&cx).unwrap();
    assert_eq!(player.state(), PlayerState::Idle);
    assert_eq!(cx.active_player_count(), 1);
    player.stop().unwrap();
    assert_eq!(player.state(), PlayerState::Idle);
}

/// Loading a source transitions Idle → Loading → Paused.
#[test]
fn load_transitions_to_paused() {
    common::require_audio_device();

    let cx = PlayerContext::new().unwrap();
    let player = Player::new(&cx).unwrap();
    player.set_volume(0.0).unwrap();

    let src = MemoryAudioSource::new(common::make_sine_wav(common::WavSpec {
        seconds: 0.1,
        ..Default::default()
    }));
    let meta = player.load(Box::new(src)).unwrap();
    assert!(meta.duration.is_some());
    // After load completes the worker reports Paused (decoder ready, sink
    // open, not yet played).
    assert_eq!(player.state(), PlayerState::Paused);
}

/// Play from Paused → Playing; pause back → Paused.
#[test]
fn play_pause_cycle() {
    common::require_audio_device();

    let cx = PlayerContext::new().unwrap();
    let player = Player::new(&cx).unwrap();
    player.set_volume(0.0).unwrap();

    let src = MemoryAudioSource::new(common::make_sine_wav(common::WavSpec {
        seconds: 2.0,
        ..Default::default()
    }));
    player.load(Box::new(src)).unwrap();
    assert_eq!(player.state(), PlayerState::Paused);

    player.play().unwrap();
    // Give the worker a moment to process the command and pump a frame.
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(player.state(), PlayerState::Playing);

    player.pause().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(player.state(), PlayerState::Paused);
}

/// A short source reaches `Ended`.
#[test]
fn short_source_ends() {
    common::require_audio_device();

    let cx = PlayerContext::new().unwrap();
    let player = Player::new(&cx).unwrap();
    player.set_volume(0.0).unwrap();

    let src = MemoryAudioSource::new(common::make_sine_wav(common::WavSpec {
        seconds: 0.2,
        ..Default::default()
    }));
    player.load(Box::new(src)).unwrap();
    player.play().unwrap();

    // Spin briefly until the worker reports Ended (or time out).
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while player.state() != PlayerState::Ended {
        if std::time::Instant::now() > deadline {
            panic!(
                "player did not reach Ended within 3s; current state = {:?}",
                player.state()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
