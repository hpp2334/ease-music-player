//! Container/codec coverage tests against tiny committed fixtures.
//!
//! Unlike `real_files.rs` (network-fetched, skip-when-absent), the fixtures
//! under `tests/fixtures/` are checked in: synthetic 2-second 16 kHz mono
//! sine tones, one per container, generated with ffmpeg. They are small
//! (3–64 KB) but exercise the real demuxer + decoder paths:
//!
//! - `tone-2s-m4a.m4a` — AAC in ISO-MP4. **Regression guard:** symphonia's
//!   isomp4 reader does not declare `channels` for AAC tracks (only
//!   PCM/ALAC sample entries get them), so `SymphoniaDecoder::open` must
//!   discover the signal spec by decoding the first packet. Before that
//!   logic existed, every `.m4a` failed to open with
//!   "unsupported format: unknown channel layout".
//! - `tone-2s-mp3.mp3` / `tone-2s-flac.flac` / `tone-2s-wav.wav` — the
//!   other common containers, used to cover [`Decoder::seek`] per format.
//!
//! Everything here is headless: no output device is touched.

use std::time::Duration;

use cantode::{AudioSource, Decoder, DecoderFactory, MemoryAudioSource, SymphoniaDecoderFactory};

const M4A: &[u8] = include_bytes!("fixtures/tone-2s-m4a.m4a");
const MP3: &[u8] = include_bytes!("fixtures/tone-2s-mp3.mp3");
const FLAC: &[u8] = include_bytes!("fixtures/tone-2s-flac.flac");
const WAV: &[u8] = include_bytes!("fixtures/tone-2s-wav.wav");

fn open(bytes: &[u8]) -> Box<dyn Decoder> {
    let source: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(bytes.to_vec()));
    SymphoniaDecoderFactory::new()
        .open(source)
        .unwrap_or_else(|e| panic!("decoder open failed: {e:?}"))
}

/// AAC-in-MP4 opens and reports a full signal spec even though the isomp4
/// demuxer leaves `channels` undeclared for AAC tracks.
#[test]
fn m4a_aac_opens_with_undeclared_channel_layout() {
    let dec = open(M4A);

    let format = dec.format();
    assert_eq!(format.channels, 1, "fixture is mono");
    assert_eq!(format.sample_rate, 16_000, "fixture is 16 kHz");

    let meta = dec.metadata();
    let duration = meta
        .duration
        .unwrap_or_else(|| panic!("m4a: duration not reported (was None)"));
    let ms = duration.as_millis();
    assert!(
        (1_800..=2_200).contains(&ms),
        "m4a: duration {ms}ms outside the 2s fixture tolerance"
    );
}

/// The first packet decoded during spec discovery must not be lost: the
/// first `next_frame` returns it, and the reader continues with the
/// second packet afterwards (audio from t=0 onward, no gap).
#[test]
fn m4a_spec_discovery_frame_is_returned_first() {
    let mut dec = open(M4A);

    let first = dec
        .next_frame()
        .expect("first next_frame errored")
        .expect("first next_frame returned None");
    assert!(first.frames > 0, "m4a: first frame carries no audio");
    assert_eq!(
        first.data.len(),
        first.frames * dec.format().channels as usize,
        "m4a: interleaved data length != frames * channels"
    );
    assert!(
        first.timestamp < Duration::from_millis(200),
        "m4a: first frame timestamp {:?} should be at the start of the stream",
        first.timestamp
    );

    // And the stream continues — the reader was left positioned after the
    // spec-discovery packet, so the next frame is more audio (not a
    // repeat, not EOF).
    let second = dec
        .next_frame()
        .expect("second next_frame errored")
        .expect("second next_frame returned None — stream ended after one frame?");
    assert!(second.frames > 0, "m4a: second frame carries no audio");
    assert!(
        second.timestamp > first.timestamp,
        "m4a: second frame ({:?}) should follow the first ({:?})",
        second.timestamp,
        first.timestamp
    );
}

/// [`Decoder::seek`] lands near the target for each common container.
/// Decodes a little audio first (the realistic play-then-seek path), seeks
/// to half the duration, and asserts the next frame's timestamp jumped.
#[test]
fn seek_to_midpoint_across_containers() {
    for (name, bytes) in [("mp3", MP3), ("flac", FLAC), ("wav", WAV), ("m4a", M4A)] {
        let mut dec = open(bytes);
        let duration = dec
            .metadata()
            .duration
            .unwrap_or_else(|| panic!("{name}: duration not reported"));
        assert!(
            duration >= Duration::from_secs(1),
            "{name}: fixture too short to seek-test"
        );

        // Play a moment: consume frames until we are ~0.25s in.
        let mut pre_seek = Duration::ZERO;
        while pre_seek < Duration::from_millis(250) {
            match dec.next_frame().unwrap_or_else(|e| panic!("{name}: decode: {e:?}")) {
                Some(frame) => pre_seek = frame.timestamp,
                None => panic!("{name}: stream ended before seeking"),
            }
        }

        let target = duration / 2;
        let landed = dec
            .seek(target)
            .unwrap_or_else(|e| panic!("{name}: seek({target:?}) failed: {e:?}"));

        let frame = dec
            .next_frame()
            .unwrap_or_else(|e| panic!("{name}: decode after seek: {e:?}"))
            .unwrap_or_else(|| panic!("{name}: no audio after seek"));

        assert!(
            frame.timestamp > pre_seek,
            "{name}: after seeking to {target:?} the next frame ({:?}) did not advance past \
             the pre-seek position ({pre_seek:?})",
            frame.timestamp
        );
        assert!(
            frame.timestamp.abs_diff(landed) < Duration::from_millis(150),
            "{name}: next-frame timestamp {:?} disagrees with seek landing point {landed:?}",
            frame.timestamp
        );
        let drift = frame.timestamp.abs_diff(target);
        assert!(
            drift < Duration::from_millis(600),
            "{name}: seek landed {drift:?} away from the target {target:?} \
             (duration {duration:?})",
        );
        println!(
            "[test] {name}: seek({target:?}) -> landed {landed:?}, next frame {frame:?} @ {:?}",
            frame.timestamp
        );
    }
}
