//! Tests for `cantode::probe_metadata`.

use cantode::{AudioSource, MemoryAudioSource, PlayerContext, probe_metadata};

mod common;

#[test]
fn probe_wav_reports_format_and_duration() {
    let cx = PlayerContext::new().unwrap();
    let probe_src: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(common::make_sine_wav(
        common::WavSpec {
            sample_rate: 44_100,
            channels: 1,
            seconds: 1.0,
            freq: 440.0,
        },
    )));

    let meta = probe_metadata(&cx, probe_src).unwrap();
    assert_eq!(meta.format.sample_rate, 44_100);
    assert_eq!(meta.format.channels, 1);
    assert!(meta.duration.is_some(), "duration should be known for WAV");
    let dur = meta.duration.unwrap();
    // Allow ±10ms tolerance for rounding.
    assert!(
        dur.as_millis() >= 990 && dur.as_millis() <= 1010,
        "duration was {dur:?}, expected ~1s"
    );
}

#[test]
fn probe_total_samples_is_consistent_with_duration() {
    let cx = PlayerContext::new().unwrap();
    let probe_src: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(common::make_sine_wav(
        common::WavSpec {
            sample_rate: 48_000,
            channels: 2,
            seconds: 0.5,
            freq: 220.0,
        },
    )));
    let meta = probe_metadata(&cx, probe_src).unwrap();

    assert_eq!(meta.format.channels, 2);
    assert_eq!(meta.format.sample_rate, 48_000);
    if let (Some(dur), Some(total)) = (meta.duration, meta.total_samples) {
        let expected = (dur.as_secs_f64() * meta.format.sample_rate as f64).round() as u64;
        // Allow 1-sample rounding tolerance.
        assert!(
            (total as i64 - expected as i64).abs() <= 1,
            "total_samples={total}, expected≈{expected}"
        );
    }
}
