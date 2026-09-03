//! Network-behavior tests: playback when the byte source behaves like a
//! network.
//!
//! The engine itself never talks to a network — an [`AudioSource`] with
//! network *manners* is indistinguishable from one. [`ScriptedSource`]
//! below serves a WAV from RAM but in small chunks, with per-chunk delay,
//! a one-shot stall gate, and scheduled read failures; [`CaptureSink`]
//! (injected via the public
//! [`AudioSinkFactory`](cantode::AudioSinkFactory) /
//! [`PlayerConfig::audio_sink_factory`](cantode::PlayerConfig) seam)
//! records every sample the worker pushes. Together they pin:
//!
//! - **Behavior (B-tests):** chunked delivery plays through to `Ended`;
//!   seek re-issues reads at the container-derived byte offset (the
//!   "caller loads by offset" contract); a stall freezes position and
//!   defers commands; a persistent read error keeps the player
//!   `Playing` but silent; error-then-EOF masquerades as `Ended`.
//! - **Output correctness (O-tests):** the captured samples are
//!   **bit-exact** against a reference decode of the same bytes from a
//!   `MemoryAudioSource` — chunking, delays, seeks, and failures cannot
//!   corrupt what reaches the sink. Symphonia is deterministic, so the
//!   reference decode is a valid oracle.
//!
//! Everything here is device-free: no cpal output device is opened. The
//! real-device path stays covered by `tests/playback.rs`.
//!
//! Two *characterized warts* (documented, not fixed here — see the
//! follow-ups note at the bottom):
//!
//! - `error_then_eof_*`: a source that errors once and then reports EOF
//!   produces a premature `Ended` even though data remained — the exact
//!   shape of a failed chunk over HTTP in the embedding app. (Sources
//!   built on `BufferedSource` retry lying EOFs themselves, so this
//!   only bites hand-rolled `AudioSource`s like this one.)
//! - `AudioSource::is_infinite` is never consulted by the engine.
//!
//! Note on `Buffering`: `ScriptedSource` implements neither
//! `readiness()` nor `set_read_deadline` (defaults), so a stall here
//! still parks the worker — the classic freeze characterization. The
//! `Buffering` morph itself is covered in `tests/buffered_source.rs`
//! against a source that opts in.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use cantode::{
    AudioFormat, AudioSink, AudioSinkFactory, AudioSource, CantodeError, ChannelEventSink,
    MemoryAudioSource, Player, PlayerConfig, PlayerContext, PlayerEvent, PlayerState,
};

mod common;

use common::{RATE, reference_decode, wait_for_ended, wait_for_quiet, wait_until};

/// Mono 16-bit 44.1 kHz → bytes per second of audio; WAV data starts at
/// byte 44. Used to translate seek targets into expected byte offsets.
const DATA_START: u64 = 44;

fn wav(seconds: f32) -> Vec<u8> {
    common::make_sine_wav(common::WavSpec {
        seconds,
        ..Default::default()
    })
}

// ============================================================================
// ScriptedSource — an AudioSource with network manners
// ============================================================================

/// How a scheduled failure behaves once it fires.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FailMode {
    /// Fail one read, then serve normally again.
    Once,
    /// Fail every read from the trigger point on.
    Forever,
    /// Fail one read, then report EOF on every subsequent read. This is
    /// the byte-level shape of a failed HTTP chunk in the embedding app's
    /// `MusicAudioSource`.
    ThenEof,
}

struct FailPlan {
    at_byte: u64,
    mode: FailMode,
    fired: bool,
}

/// A seekable byte source over in-RAM data that serves it in small chunks
/// with configurable delay, a one-shot stall gate, and scheduled failures
/// — i.e. how a network source behaves from the decoder's point of view.
struct ScriptedSource {
    data: Vec<u8>,
    pos: Mutex<u64>,
    chunk_size: usize,
    chunk_delay: Duration,
    fail: Mutex<Option<FailPlan>>,
    /// `(at_byte, receiver)` — the first read at/after `at_byte` blocks
    /// in `recv()` until the sender signals (or is dropped).
    gate: Mutex<Option<(u64, mpsc::Receiver<()>)>>,
    unknown_len: bool,
    /// Bytes handed out so far (successful serves only).
    bytes_served: Mutex<u64>,
    /// Source offsets of successful serves, in order.
    read_positions: Mutex<Vec<u64>>,
    /// `(SeekFrom, resolved absolute offset)` in order.
    seeks: Mutex<Vec<(SeekFrom, u64)>>,
}

impl ScriptedSource {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            pos: Mutex::new(0),
            chunk_size: 8 * 1024,
            chunk_delay: Duration::ZERO,
            fail: Mutex::new(None),
            gate: Mutex::new(None),
            unknown_len: false,
            bytes_served: Mutex::new(0),
            read_positions: Mutex::new(Vec::new()),
            seeks: Mutex::new(Vec::new()),
        }
    }

    /// Max bytes served per `read` call.
    fn chunk(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Artificial per-read latency (network RTT-ish).
    fn delay(mut self, d: Duration) -> Self {
        self.chunk_delay = d;
        self
    }

    /// Report unknown length (`len() == None`), like an unseekable /
    /// chunked HTTP response.
    fn unknown_len(mut self) -> Self {
        self.unknown_len = true;
        self
    }

    /// Schedule a read failure at (or after) `at_byte`.
    fn fail_at(&mut self, at_byte: u64, mode: FailMode) {
        *self.fail.lock().unwrap() = Some(FailPlan {
            at_byte,
            mode,
            fired: false,
        });
    }

    /// Arm the one-shot stall gate: the first read at/after `at_byte`
    /// blocks until the returned sender signals.
    fn stall_after(&mut self, at_byte: u64) -> mpsc::Sender<()> {
        let (tx, rx) = mpsc::channel();
        *self.gate.lock().unwrap() = Some((at_byte, rx));
        tx
    }

    fn bytes_served(&self) -> u64 {
        *self.bytes_served.lock().unwrap()
    }

    fn read_positions(&self) -> Vec<u64> {
        self.read_positions.lock().unwrap().clone()
    }

    fn seek_resolutions(&self) -> Vec<u64> {
        self.seeks.lock().unwrap().iter().map(|(_, r)| *r).collect()
    }
}

impl Read for ScriptedSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let pos = *self.pos.lock().unwrap();

        // One-shot stall gate: block this read until released.
        {
            let mut gate = self.gate.lock().unwrap();
            if let Some((at, rx)) = gate.take() {
                if pos >= at {
                    let _ = rx.recv(); // blocks; a dropped sender also releases
                } else {
                    *gate = Some((at, rx)); // not reached yet — re-arm
                }
            }
        }

        enum Action {
            Serve,
            Fail,
            Eof,
        }
        let action = {
            let mut fail = self.fail.lock().unwrap();
            match fail.as_mut() {
                Some(plan) if plan.fired => match plan.mode {
                    FailMode::Forever => Action::Fail,
                    FailMode::ThenEof => Action::Eof,
                    FailMode::Once => Action::Serve, // plan cleared on fire
                },
                Some(plan) if pos >= plan.at_byte => {
                    plan.fired = true;
                    Action::Fail
                }
                _ => Action::Serve,
            }
        };
        match action {
            Action::Fail => {
                // A `Once` plan has now delivered its hiccup — clear it so
                // later reads serve normally.
                let mut fail = self.fail.lock().unwrap();
                if matches!(fail.as_ref().map(|p| p.mode), Some(FailMode::Once)) {
                    *fail = None;
                }
                return Err(io::Error::other("scripted read failure"));
            }
            Action::Eof => return Ok(0),
            Action::Serve => {}
        }

        if self.chunk_delay > Duration::ZERO {
            std::thread::sleep(self.chunk_delay);
        }

        let pos_u = pos as usize;
        if pos_u >= self.data.len() {
            return Ok(0);
        }
        let n = self.chunk_size.min(buf.len()).min(self.data.len() - pos_u);
        buf[..n].copy_from_slice(&self.data[pos_u..pos_u + n]);
        *self.pos.lock().unwrap() = pos + n as u64;
        *self.bytes_served.lock().unwrap() += n as u64;
        self.read_positions.lock().unwrap().push(pos);
        Ok(n)
    }
}

impl Seek for ScriptedSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let cur = *self.pos.lock().unwrap() as i64;
        let total = self.data.len() as i64;
        let resolved = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::Current(d) => cur + d,
            SeekFrom::End(d) => total + d,
        };
        let target = resolved.clamp(0, total) as u64;
        self.seeks.lock().unwrap().push((pos, target));
        *self.pos.lock().unwrap() = target;
        Ok(target)
    }
}

impl AudioSource for ScriptedSource {
    fn len(&self) -> Option<u64> {
        if self.unknown_len {
            None
        } else {
            Some(self.data.len() as u64)
        }
    }
}

// ============================================================================
// Harness + helpers
// ============================================================================

fn capture_factory(pace: bool) -> (Arc<Mutex<common::CaptureState>>, AudioSinkFactory) {
    common::capture_factory(pace)
}

struct Harness {
    _cx: PlayerContext,
    player: Player,
    capture: Arc<Mutex<common::CaptureState>>,
    events: mpsc::Receiver<PlayerEvent>,
    /// Kept so test code can keep scripting the source after `load`
    /// moved it into the player (the player only needs `Box<dyn
    /// AudioSource>`; the logs are shared behind `Arc`).
    source: Arc<Mutex<ScriptedSource>>,
}

/// Build a device-free player around a `ScriptedSource` wrapped for
/// sharing: `load` takes ownership of the `Box`, so the source itself
/// lives behind this handle. `Box<AudioSource>` is not cloneable — instead
/// the *logs* are shared via the struct's `Arc<Mutex<…>>` fields, and
/// `load` is handed a thin forwarding wrapper.
struct SharedSource {
    inner: Arc<Mutex<ScriptedSource>>,
}

impl Read for SharedSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().read(buf)
    }
}

impl Seek for SharedSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.lock().unwrap().seek(pos)
    }
}

impl AudioSource for SharedSource {
    fn len(&self) -> Option<u64> {
        self.inner.lock().unwrap().len()
    }
}

fn harness_with(source: Arc<Mutex<ScriptedSource>>, pace: bool) -> Harness {
    let cx = PlayerContext::new().unwrap();
    let (capture, factory) = capture_factory(pace);
    let event_sink = Arc::new(ChannelEventSink::new(1024));
    let events = event_sink.subscribe();

    let player = Player::with_config(
        &cx,
        PlayerConfig::default()
            .audio_sink_factory(factory)
            .event_sink(Some(event_sink)),
    )
    .expect("player construction failed");

    Harness {
        _cx: cx,
        player,
        capture,
        events,
        source,
    }
}

fn captured_samples(capture: &Arc<Mutex<common::CaptureState>>) -> Vec<f32> {
    capture.lock().unwrap().samples.clone()
}

/// Split the captured samples into segments delimited by `flush` calls
/// (each seek flushes the sink, so segments = inter-seek play spans).
/// Empty segments are dropped (e.g. the leading span before any seek).
fn captured_segments(capture: &Arc<Mutex<common::CaptureState>>) -> Vec<Vec<f32>> {
    captured_segments_raw(capture)
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect()
}

/// Like [`captured_segments`] but keeping empty segments — the *current*
/// span must be addressable even before it has any samples, or "wait for
/// the current segment to grow" would pass vacuously on the previous
/// segment.
fn captured_segments_raw(capture: &Arc<Mutex<common::CaptureState>>) -> Vec<Vec<f32>> {
    let st = capture.lock().unwrap();
    let mut segs: Vec<Vec<f32>> = vec![Vec::new()];
    for ev in &st.events {
        match ev {
            common::CapEvent::Write { offset, len } => {
                let seg = segs.last_mut().expect("always one open segment");
                seg.extend_from_slice(&st.samples[*offset..*offset + len]);
            }
            common::CapEvent::Flush => segs.push(Vec::new()),
            _ => {}
        }
    }
    segs
}

/// Locate `needle` in `haystack`, searching a ±`window` sample window
/// around `expected`. Returns the match offset (needle[0] must equal
/// haystack[offset..] for its full length — verified by the caller).
fn aligned_offset(
    haystack: &[f32],
    needle: &[f32],
    expected: usize,
    window: usize,
) -> Option<usize> {
    let head_len = needle.len().min(64);
    let head = &needle[..head_len];
    let lo = expected.saturating_sub(window);
    let hi = (haystack.len() + 1)
        .saturating_sub(head_len)
        .min(expected + window + 1);
    (lo..hi).find(|&i| haystack[i..i + head_len] == *head)
}

/// Assert a captured segment equals the reference decode starting within
/// ±`window` samples of `expected_sample`.
fn assert_segment_matches_reference(
    reference: &[f32],
    segment: &[f32],
    expected_sample: usize,
    ctx: &str,
) {
    let off = aligned_offset(reference, segment, expected_sample, 4096)
        .unwrap_or_else(|| panic!("segment not found near sample {expected_sample} ({ctx})"));
    assert_eq!(
        &reference[off..off + segment.len()],
        segment,
        "segment content mismatch near sample {off} ({ctx})"
    );
}

// ============================================================================
// T0 — ScriptedSource sanity (no player, no device)
// ============================================================================

#[test]
fn scripted_chunked_reads_reconstruct_the_file() {
    let data = wav(1.0);
    let mut src = ScriptedSource::new(data.clone()).chunk(8 * 1024);
    let mut buf = Vec::new();
    src.read_to_end(&mut buf).unwrap();
    assert_eq!(buf, data);

    let positions = src.read_positions();
    assert!(!positions.is_empty());
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "offsets must advance"
    );
}

#[test]
fn scripted_seek_records_and_serves_from_offset() {
    let data = wav(1.0);
    let mut src = ScriptedSource::new(data.clone()).chunk(4 * 1024);

    src.seek(SeekFrom::Start(10)).unwrap();
    let mut buf = [0u8; 4];
    src.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, &data[10..14]);

    src.seek(SeekFrom::Current(-2)).unwrap();
    src.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, &data[12..16]);

    src.seek(SeekFrom::End(-4)).unwrap();
    src.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, &data[data.len() - 4..]);

    assert_eq!(src.seek_resolutions().len(), 3);
}

#[test]
fn scripted_gate_blocks_until_released() {
    let data = wav(0.5);
    let mut src = ScriptedSource::new(data);
    let gate = src.stall_after(64);

    let mut first = [0u8; 64];
    src.read_exact(&mut first).unwrap(); // before the gate — not blocked

    let done = Arc::new(Mutex::new(false));
    let flag = Arc::clone(&done);
    let mut blocked_src = src;
    let handle = std::thread::spawn(move || {
        let mut buf = [0u8; 16];
        blocked_src.read_exact(&mut buf).unwrap();
        *flag.lock().unwrap() = true;
    });

    std::thread::sleep(Duration::from_millis(150));
    assert!(!*done.lock().unwrap(), "read past the gate must block");
    gate.send(()).unwrap();
    handle.join().expect("gated reader thread");
    assert!(*done.lock().unwrap(), "read completes after release");
}

#[test]
fn scripted_fail_modes() {
    let data = wav(0.5);
    // chunk 64 + trigger on a chunk boundary, so exactly the reads at/after
    // byte 64 fail.
    let mut src = ScriptedSource::new(data.clone()).chunk(64);
    src.fail_at(64, FailMode::Once);
    let mut buf = [0u8; 200];
    let mut got = 0;
    while got < 200 {
        match src.read(&mut buf[got..]) {
            Ok(0) => panic!("premature EOF"),
            Ok(n) => got += n,
            Err(_) => {} // the single scripted failure
        }
    }
    assert_eq!(&buf, &data[..200]);

    // Forever: every read at/after the trigger errors.
    let mut src = ScriptedSource::new(data.clone()).chunk(64);
    src.fail_at(64, FailMode::Forever);
    let mut buf = [0u8; 64];
    assert!(src.read_exact(&mut buf).is_ok()); // bytes 0..64
    let mut errors = 0;
    for _ in 0..5 {
        if src.read(&mut buf).is_err() {
            errors += 1;
        }
    }
    assert_eq!(errors, 5, "Forever must fail every read from the trigger");

    // ThenEof: one error, then clean EOF.
    let mut src = ScriptedSource::new(data).chunk(64);
    src.fail_at(64, FailMode::ThenEof);
    assert!(src.read_exact(&mut [0u8; 64]).is_ok());
    assert!(src.read(&mut buf).is_err(), "first read at trigger fails");
    assert_eq!(src.read(&mut buf).unwrap(), 0, "subsequent reads are EOF");
}

// ============================================================================
// C0 — CaptureSink sanity (no player, no device)
// ============================================================================

#[test]
fn capture_sink_records_calls_and_shared_state() {
    let (state, factory) = capture_factory(false);
    assert_eq!(state.lock().unwrap().events.len(), 0);

    let mut a = factory().unwrap();
    let fmt = AudioFormat::new(2, 48_000);
    assert_eq!(a.start(fmt).unwrap().channels, 2);
    a.write(&[0.5, -0.5], Duration::ZERO).unwrap();
    a.flush().unwrap();
    a.pause().unwrap();
    a.resume().unwrap();
    a.set_volume(0.0).unwrap();
    a.stop().unwrap();

    // A second sink from the same factory (next `load`) shares the state.
    let mut b = factory().unwrap();
    b.start(AudioFormat::new(2, 48_000)).unwrap();
    b.write(&[1.0, 1.0], Duration::ZERO).unwrap();

    let st = state.lock().unwrap();
    assert_eq!(st.samples, vec![0.5, -0.5, 1.0, 1.0]);
    let started: Vec<(u16, u32)> = st
        .events
        .iter()
        .filter_map(|e| match e {
            common::CapEvent::Start {
                channels,
                sample_rate,
            } => Some((*channels, *sample_rate)),
            _ => None,
        })
        .collect();
    assert_eq!(started, vec![(2, 48_000), (2, 48_000)]);
    let calls: Vec<&str> = st
        .events
        .iter()
        .map(|e| match e {
            common::CapEvent::Start { .. } => "start",
            common::CapEvent::Write { .. } => "write",
            common::CapEvent::Flush => "flush",
            common::CapEvent::Pause => "pause",
            common::CapEvent::Resume => "resume",
            common::CapEvent::Stop => "stop",
            common::CapEvent::SetVolume => "volume",
        })
        .collect();
    assert_eq!(
        calls,
        vec![
            "start", "write", "flush", "pause", "resume", "volume", "stop", "start", "write"
        ]
    );
}

#[test]
fn capture_sink_pacing_throttles_to_realtime() {
    let mut sink =
        common::CaptureSink::shared(&Arc::new(Mutex::new(common::CaptureState::default())), true);
    sink.start(AudioFormat::new(1, RATE)).unwrap();

    // A real device accepts writes until its buffer fills: the first
    // 0.5s chunk fits within the emulated buffer and returns instantly…
    let t0 = Instant::now();
    let half_second = vec![0.0; RATE as usize / 2];
    sink.write(&half_second, Duration::ZERO).unwrap();
    assert!(
        t0.elapsed() < Duration::from_millis(150),
        "buffered chunk must be accepted quickly"
    );

    // …but writing beyond the buffer blocks until the device drains.
    sink.write(&half_second, Duration::ZERO).unwrap(); // 1.0s total vs 0.5s budget
    assert!(
        t0.elapsed() >= Duration::from_millis(400),
        "over-buffer chunk must pace to real time (took {:?})",
        t0.elapsed()
    );
}

// ============================================================================
// B-tests — behavior over a network-mannered source (device-free)
// ============================================================================

fn chunked_source(data: &[u8], chunk: usize, delay: Duration) -> Arc<Mutex<ScriptedSource>> {
    Arc::new(Mutex::new(
        ScriptedSource::new(data.to_vec()).chunk(chunk).delay(delay),
    ))
}

fn load_shared(h: &Harness) -> cantode::Metadata {
    h.player
        .load(Box::new(SharedSource {
            inner: Arc::clone(&h.source),
        }))
        .expect("load")
}

#[test]
fn slow_source_plays_through_to_ended() {
    let data = wav(2.0);
    let h = harness_with(
        chunked_source(&data, 8 * 1024, Duration::from_millis(5)),
        false,
    );
    let meta = load_shared(&h);
    assert!(meta.duration.is_some());

    h.player.play().unwrap();
    assert!(
        wait_for_ended(&h.events, Duration::from_secs(10)),
        "must reach Ended"
    );
    assert_eq!(h.player.state(), PlayerState::Ended);
    assert!(wait_until(Duration::from_secs(1), || {
        captured_samples(&h.capture).len() as u64 > 0
    }));
}

#[test]
fn seek_rereads_from_byte_offset() {
    let data = wav(4.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    load_shared(&h);
    h.player.play().unwrap();

    // Let ~0.3s play, then seek forward to 2s.
    wait_until(Duration::from_secs(3), || {
        h.player.position() > Duration::from_millis(300)
    });
    let actual = h.player.seek(Duration::from_secs(2)).unwrap();
    assert!(
        actual >= Duration::from_millis(1900) && actual <= Duration::from_millis(2100),
        "seek returned {actual:?}"
    );

    // The source saw a byte seek to the container-derived offset:
    // 44 + 2s × 88200 B/s (mono 16-bit), within one chunk of slop.
    let expected_off = DATA_START + 2 * (RATE as u64 * 2);
    let resolutions = h.source.lock().unwrap().seek_resolutions();
    assert!(
        resolutions
            .iter()
            .any(|r| r.abs_diff(expected_off) <= 8 * 1024),
        "no byte seek near {expected_off}: {resolutions:?}"
    );

    // All reads after that seek resume at/after the new offset; precise
    // re-read correctness is covered sample-exactly by the O-tests below.

    // The sink was flushed for the seek and post-seek audio flowed (the
    // flush arrives with the seek reply; the first post-seek write lands
    // on the worker's next pump tick).
    assert!(
        wait_until(Duration::from_secs(3), || {
            captured_segments(&h.capture).len() >= 2
        }),
        "flush must split the capture into pre/post-seek segments"
    );

    // Backward seek: to 0.5s — re-fetched from near the start.
    h.player.seek(Duration::from_millis(500)).unwrap();
    let back = DATA_START + (RATE as u64 * 2) / 2;
    let resolutions = h.source.lock().unwrap().seek_resolutions();
    assert!(
        resolutions.iter().any(|r| r.abs_diff(back) <= 8 * 1024),
        "no byte seek near {back}: {resolutions:?}"
    );

    h.player.stop().unwrap();
}

#[test]
fn stall_freezes_position_and_defers_pause() {
    let data = wav(4.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    load_shared(&h);
    h.player.play().unwrap();

    // Arm the gate at ~2.5s of audio; playback stalls once the read
    // cursor arrives (decode keeps advancing through buffered bytes for
    // a moment, then freezes).
    let gate_at = DATA_START + 5 * (RATE as u64 * 2) / 2;
    let gate = h.source.lock().unwrap().stall_after(gate_at);

    // The freeze detector: position stops advancing for 300ms.
    let frozen = wait_for_quiet(Duration::from_secs(6), Duration::from_millis(300), || {
        h.player.position()
    })
    .expect("position must freeze on stall");
    assert!(
        frozen > Duration::from_secs(1),
        "should have played well into the file before stalling ({frozen:?})"
    );
    assert_eq!(h.player.state(), PlayerState::Playing);

    // …and fire-and-forget commands queue behind the blocked read.
    h.player.pause().unwrap(); // returns immediately
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        h.player.state(),
        PlayerState::Playing,
        "pause must be deferred while the worker is blocked in a read"
    );

    // Release: the pause lands.
    gate.send(()).unwrap();
    assert!(
        wait_until(Duration::from_secs(3), || h.player.state()
            == PlayerState::Paused),
        "deferred pause must take effect after release"
    );

    h.player.stop().unwrap();
}

#[test]
fn persistent_read_error_stays_playing_silent() {
    let data = wav(3.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    let fail_at = DATA_START + RATE as u64 * 2; // ~1s
    h.source.lock().unwrap().fail_at(fail_at, FailMode::Forever);
    load_shared(&h);
    h.player.play().unwrap();

    // Decode reaches the failure point; the worker skips + retries, so
    // the position freezes (after buffered bytes drain).
    let frozen = wait_for_quiet(Duration::from_secs(6), Duration::from_millis(300), || {
        h.player.position()
    })
    .expect("position must freeze when the source starts erroring");
    assert!(
        frozen > Duration::from_millis(500) && frozen < Duration::from_secs(2),
        "froze at {frozen:?}, expected near the 1s failure point"
    );
    assert_eq!(h.player.state(), PlayerState::Playing);
    assert!(
        !wait_for_ended(&h.events, Duration::from_millis(300)),
        "persistent errors must not emit Ended"
    );

    // The worker loop stays responsive between retries.
    h.player.pause().unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || h.player.state()
            == PlayerState::Paused),
        "pause must work while the source is erroring"
    );

    h.player.stop().unwrap();
}

/// CHARACTERIZATION (known wart): a source that errors once and then
/// reports EOF produces a premature `Ended` — the byte-level shape of a
/// dropped HTTP chunk in the embedding app. The engine cannot currently
/// distinguish this from a genuine end of stream.
#[test]
fn error_then_eof_emits_phantom_ended() {
    let data = wav(3.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    let fail_at = DATA_START + RATE as u64 * 2; // ~1s into a 3s file
    h.source.lock().unwrap().fail_at(fail_at, FailMode::ThenEof);
    load_shared(&h);
    h.player.play().unwrap();

    assert!(
        wait_for_ended(&h.events, Duration::from_secs(5)),
        "error-then-EOF surfaces as Ended"
    );
    assert_eq!(h.player.state(), PlayerState::Ended);
    assert!(
        h.player.position() < Duration::from_secs(2),
        "ended far short of the 3s duration (position: {:?})",
        h.player.position()
    );
}

#[test]
fn unknown_length_source_loads_and_seeks() {
    let data = wav(2.0);
    let h = harness_with(
        Arc::new(Mutex::new(
            ScriptedSource::new(data).chunk(8 * 1024).unknown_len(),
        )),
        false,
    );
    let meta = load_shared(&h);
    // The WAV header carries its own sizes; duration survives unknown
    // source length.
    assert!(meta.duration.is_some());

    let actual = h.player.seek(Duration::from_secs(1)).unwrap();
    assert!(
        actual.abs_diff(Duration::from_secs(1)) <= Duration::from_millis(50),
        "seek landed at {actual:?}"
    );
    assert_eq!(h.player.position(), actual);

    h.player.play().unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        h.player.position() > Duration::from_millis(1100)
    }));
    h.player.stop().unwrap();
}

#[test]
fn failing_sink_factory_fails_load_into_error_state() {
    let cx = PlayerContext::new().unwrap();
    let factory: AudioSinkFactory =
        Arc::new(|| Err(CantodeError::Internal("no sink for you".into())));
    let player =
        Player::with_config(&cx, PlayerConfig::default().audio_sink_factory(factory)).unwrap();

    let err = player
        .load(Box::new(MemoryAudioSource::new(wav(1.0))))
        .unwrap_err();
    assert!(matches!(err, CantodeError::Internal(_)));
    assert_eq!(player.state(), PlayerState::Error);
}

// ============================================================================
// O-tests — output correctness through the full player path
// ============================================================================

#[test]
fn chunked_output_is_bit_exact() {
    let data = wav(2.0);
    let h = harness_with(
        chunked_source(&data, 8 * 1024, Duration::from_millis(2)),
        false,
    );
    load_shared(&h);
    h.player.play().unwrap();

    assert!(
        wait_for_ended(&h.events, Duration::from_secs(10)),
        "must reach Ended"
    );
    // Give the final pump iterations a moment, then compare.
    std::thread::sleep(Duration::from_millis(100));

    let captured = captured_samples(&h.capture);
    let reference = reference_decode(&data);
    assert!(!captured.is_empty());
    assert_eq!(
        captured.len(),
        reference.len(),
        "captured sample count must match the reference decode"
    );
    assert_eq!(captured, reference, "captured PCM must be bit-exact");
}

#[test]
fn seek_output_matches_reference_segment() {
    let data = wav(4.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    load_shared(&h);

    // Seek before any playback: everything captured after the flush
    // belongs to the seek target.
    let actual = h.player.seek(Duration::from_secs(2)).unwrap();
    h.player.play().unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        captured_samples(&h.capture).len() >= 3 * RATE as usize / 2 // ≥ 1.5s
    }));
    h.player.stop().unwrap();

    let captured = captured_samples(&h.capture);
    let reference = reference_decode(&data);
    let expected_sample = (actual.as_secs_f64() * RATE as f64).round() as usize;
    assert_segment_matches_reference(&reference, &captured, expected_sample, "forward seek");
}

#[test]
fn back_and_forth_seek_output_stays_correct() {
    let data = wav(4.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    load_shared(&h);

    let rate = RATE as f64;
    let stops = [
        (Duration::from_secs(3), 0.2),
        (Duration::from_millis(500), 0.2),
        (Duration::from_millis(2500), 0.25),
    ];
    let mut landed = Vec::new();
    for &(target, _) in &stops {
        let actual = h.player.seek(target).unwrap();
        landed.push(actual);
        h.player.play().unwrap();
        // Wait on the CURRENT segment (flushes split them; the raw view
        // keeps the empty current span addressable), not the total.
        assert!(
            wait_until(Duration::from_secs(5), || {
                captured_segments_raw(&h.capture)
                    .last()
                    .is_some_and(|s| s.len() >= 5000)
            }),
            "segment after seek to {target:?} produced no audio"
        );
        h.player.pause().unwrap();
        assert!(wait_until(Duration::from_secs(2), || {
            h.player.state() == PlayerState::Paused
        }));
    }
    h.player.stop().unwrap();

    let segments = captured_segments(&h.capture);
    assert_eq!(segments.len(), stops.len(), "one segment per seek");
    let reference = reference_decode(&data);
    for i in 0..stops.len() {
        let play_secs = stops[i].1;
        let actual = &landed[i];
        let seg = &segments[i];
        let expected = (actual.as_secs_f64() * rate).round() as usize;
        assert!(
            seg.len() as f64 >= play_secs * rate * 0.35,
            "segment too short: {} samples",
            seg.len()
        );
        assert_segment_matches_reference(&reference, seg, expected, "back-and-forth");
    }
}

#[test]
fn paced_sink_applies_backpressure_to_the_source() {
    let data = wav(4.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), true);
    load_shared(&h);
    h.player.play().unwrap();

    // ~1s of wall time, then check the source has NOT been drained.
    assert!(
        wait_until(Duration::from_secs(5), || {
            captured_samples(&h.capture).len() >= RATE as usize
        }),
        "should capture ~1s of audio"
    );
    std::thread::sleep(Duration::from_millis(300));

    let captured = captured_samples(&h.capture);
    assert!(
        captured.len() <= 2 * RATE as usize,
        "capture must track real time ({} samples)",
        captured.len()
    );
    let served = h.source.lock().unwrap().bytes_served();
    assert!(
        served < data.len() as u64 - 8 * 1024,
        "source must not be fully consumed mid-play ({served} of {} bytes)",
        data.len()
    );

    h.player.stop().unwrap();
}

#[test]
fn pause_gates_output_resume_reopens_it() {
    let data = wav(2.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    load_shared(&h);
    h.player.play().unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        captured_samples(&h.capture).len() >= RATE as usize / 2 // 0.5s
    }));

    h.player.pause().unwrap();
    assert!(wait_until(Duration::from_secs(2), || {
        h.player.state() == PlayerState::Paused
    }));
    let frozen = captured_samples(&h.capture).len();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        captured_samples(&h.capture).len(),
        frozen,
        "no samples while paused"
    );

    h.player.play().unwrap();
    assert!(wait_until(Duration::from_secs(2), || {
        captured_samples(&h.capture).len() > frozen
    }));
    h.player.stop().unwrap();
}

#[test]
fn error_then_eof_output_is_exact_prefix() {
    let data = wav(3.0);
    let h = harness_with(chunked_source(&data, 8 * 1024, Duration::ZERO), false);
    let fail_at = DATA_START + RATE as u64 * 2; // ~1s
    h.source.lock().unwrap().fail_at(fail_at, FailMode::ThenEof);
    load_shared(&h);
    h.player.play().unwrap();

    assert!(wait_for_ended(&h.events, Duration::from_secs(5)));
    std::thread::sleep(Duration::from_millis(100));

    // Whatever did play is exactly the reference decode's prefix…
    let captured = captured_samples(&h.capture);
    let reference = reference_decode(&data);
    assert!(captured.len() <= reference.len());
    assert_eq!(
        &reference[..captured.len()],
        &captured[..],
        "pre-failure audio must be untouched"
    );
    // …and it cut off around the failure point, not at the file's end.
    let secs = captured.len() as f64 / RATE as f64;
    assert!(
        (0.6..1.2).contains(&secs),
        "cut off at {secs:.2}s (expected ~1s)"
    );
}

// ============================================================================
// Follow-ups (deliberately NOT fixed here)
// ============================================================================
// - Distinguish source errors from clean EOF in `pump_once` so a failed
//   chunk surfaces as `PlayerEvent::Error` instead of a phantom `Ended`
//   (see `error_then_eof_emits_phantom_ended`). (Done for persistent
//   errors via the worker's dedup latch; the error-then-EOF shape still
//   masquerades because the EOF wins.)
// - Honor `AudioSource::is_infinite` (declared, but never consulted).
