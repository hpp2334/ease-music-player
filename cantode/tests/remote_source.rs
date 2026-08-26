//! Tests for [`RemoteSource`] — the cantode-owned windowed source over
//! an embedder's fetch closure.
//!
//! Same philosophy as `tests/network_source.rs`, one level up: there the
//! source itself had network manners; here the *fetch closure* does
//! (delays, a stall gate, scheduled failures, lying EOFs, unknown
//! length), and the adapter's job is to turn those into a well-behaved
//! `AudioSource`. The device-free harness ([`CaptureSink`], the wait
//! detectors, the bit-exact reference decode) is shared from
//! `common`.
//!
//! Coverage map:
//!
//! - **T0** (no player): fetch scheduling and ordering; seek inside the
//!   window hits no network; seek outside fetches at the target;
//!   generation-guarded rapid seeks; temporal `len()` and
//!   seek-from-end rejection while unknown; lying-EOF retry; retry
//!   budget → sticky error → seek recovery; bounded readahead
//!   (backpressure).
//! - **B/O** (device-free player): play-through bit-exactness; the
//!   phantom-`Ended` fix (premature close of a known-length resource is
//!   retried, not ended); stall freeze/resume with output continuity;
//!   persistent fetch errors staying `Playing`-silent.

use std::io::{Read, Seek, SeekFrom};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cantode::{
    AudioSource, ChannelEventSink, Player, PlayerConfig, PlayerContext, PlayerEvent, PlayerState,
    RemoteSource,
};

mod common;

use common::{capture_factory, reference_decode, wait_for_ended, wait_for_quiet, wait_until};

// ============================================================================
// Fetch-closure builders with network manners
// ============================================================================

/// What one fetch invocation saw, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FetchCall {
    offset: u64,
    max_len: usize,
}

/// Log of fetch invocations + bytes served, shared with the closures.
#[derive(Clone, Default)]
struct FetchLog {
    calls: Arc<Mutex<Vec<FetchCall>>>,
    bytes_served: Arc<Mutex<u64>>,
}

impl FetchLog {
    fn calls(&self) -> Vec<FetchCall> {
        self.calls.lock().unwrap().clone()
    }
    fn last_call(&self) -> Option<FetchCall> {
        self.calls.lock().unwrap().last().copied()
    }
    fn wait_for_calls(&self, n: usize) -> bool {
        wait_until(Duration::from_secs(3), || self.calls().len() >= n)
    }
    fn bytes_served(&self) -> u64 {
        *self.bytes_served.lock().unwrap()
    }
}

/// A whole-file synchronously-serving fetcher: reports the length,
/// serves from RAM in `chunk`-byte pushes, finishes with EOF. The
/// determinstic happy path — the closure delivers inside the call.
fn serving_fetch(
    data: Arc<Vec<u8>>,
    chunk: usize,
    log: FetchLog,
) -> impl Fn(u64, usize, cantode::ReplyHandle) + Send + Sync + 'static {
    move |offset, max_len, reply| {
        log.calls
            .lock()
            .unwrap()
            .push(FetchCall { offset, max_len });
        reply.set_total_len(Some(data.len() as u64));
        let mut served = *log.bytes_served.lock().unwrap();
        let end = (offset + max_len as u64).min(data.len() as u64);
        let mut at = offset;
        while at < end {
            let n = chunk.min((end - at) as usize);
            reply.push_chunk(data[at as usize..at as usize + n].to_vec());
            at += n as u64;
            served += n as u64;
        }
        *log.bytes_served.lock().unwrap() = served;
        if at >= data.len() as u64 {
            reply.finish_eof();
        }
    }
}

/// A fetcher whose first fetch *at or after* `gate_at_offset` is gated:
/// nothing is served until the gate fires (from a spawned thread, so
/// the closure itself still returns immediately — the contract the
/// prefetch thread expects). Earlier fetches serve normally, so
/// construction/load-probing is never blocked.
fn gated_fetch(
    data: Arc<Vec<u8>>,
    gate_at_offset: u64,
) -> (
    impl Fn(u64, usize, cantode::ReplyHandle) + Send + Sync + 'static,
    mpsc::Sender<()>,
    FetchLog,
) {
    let log = FetchLog::default();
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let gate_slot: Arc<Mutex<Option<mpsc::Receiver<()>>>> = Arc::new(Mutex::new(Some(gate_rx)));
    let fetch_log = log.clone();
    let fetch = move |offset: u64, max_len: usize, reply: cantode::ReplyHandle| {
        fetch_log
            .calls
            .lock()
            .unwrap()
            .push(FetchCall { offset, max_len });
        let data = Arc::clone(&data);
        let reply = reply.clone();
        let gate = if offset >= gate_at_offset {
            gate_slot.lock().unwrap().take()
        } else {
            None
        };
        std::thread::spawn(move || {
            if let Some(rx) = gate {
                // Block only the delivery, never the prefetch thread.
                let _ = rx.recv();
            }
            reply.set_total_len(Some(data.len() as u64));
            let end = (offset + max_len as u64).min(data.len() as u64);
            let mut at = offset;
            while at < end {
                let n = 8192.min((end - at) as usize);
                reply.push_chunk(data[at as usize..at as usize + n].to_vec());
                at += n as u64;
            }
            if at >= data.len() as u64 {
                reply.finish_eof();
            }
        });
    };
    (fetch, gate_tx, log)
}

// ============================================================================
// T0 — adapter mechanics, no player
// ============================================================================

fn read_n(src: &mut RemoteSource, n: usize) -> std::io::Result<Vec<u8>> {
    let mut out = vec![0u8; n];
    let mut got = 0;
    while got < n {
        let k = src.read(&mut out[got..])?;
        if k == 0 {
            out.truncate(got);
            return Ok(out);
        }
        got += k;
    }
    Ok(out)
}

#[test]
fn fetches_are_windowed_and_ordered() {
    let data: Arc<Vec<u8>> = Arc::new((0..100_000u32).map(|i| i as u8).collect());
    let log = FetchLog::default();
    let fetch = serving_fetch(Arc::clone(&data), 8 * 1024, log.clone());
    let mut src = RemoteSource::with_readahead(16 * 1024, fetch);

    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data, "full read must reconstruct the file");

    // Fetches: contiguous 16 KiB windows from 0, in order.
    let calls = log.calls();
    assert_eq!(calls[0].offset, 0);
    for w in calls.windows(2) {
        assert_eq!(w[1].offset, w[0].offset + w[0].max_len as u64);
    }
    // ...but never beyond the (reported) end.
    assert!(calls.last().unwrap().offset < data.len() as u64);
}

#[test]
fn seek_inside_window_hits_no_network() {
    // Sized to fit one readahead window: a single fetch at 0 covers the
    // whole resource and the EOF it reports means no top-up fetches —
    // so any fetch after the first would have to come from the seek.
    let data: Arc<Vec<u8>> = Arc::new((0..32 * 1024u32).map(|i| (i * 7) as u8).collect());
    let log = FetchLog::default();
    let fetch = serving_fetch(Arc::clone(&data), 8 * 1024, log.clone());
    let mut src = RemoteSource::with_readahead(32 * 1024, fetch);

    let first = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(first, data[..4 * 1024]);
    assert!(
        wait_until(Duration::from_secs(2), || log.calls().len() == 1),
        "expected exactly the initial fetch: {:?}",
        log.calls()
    );

    // Backward seek within the retained window: pure cursor move.
    src.seek(SeekFrom::Start(1024)).unwrap();
    let again = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(again, data[1024..5 * 1024]);
    assert_eq!(log.calls().len(), 1, "in-window seek must not fetch");
}

#[test]
fn seek_outside_window_fetches_at_target() {
    let data: Arc<Vec<u8>> = Arc::new((0..128 * 1024u32).map(|i| (i * 13) as u8).collect());
    let log = FetchLog::default();
    let fetch = serving_fetch(Arc::clone(&data), 4 * 1024, log.clone());
    let mut src = RemoteSource::with_readahead(8 * 1024, fetch);

    read_n(&mut src, 1024).unwrap();
    src.seek(SeekFrom::Start(60_000)).unwrap();
    let got = read_n(&mut src, 4_000).unwrap();
    assert_eq!(got, data[60_000..64_000]);
    assert!(
        log.calls().contains(&FetchCall {
            offset: 60_000,
            max_len: 8 * 1024
        }),
        "expected a fetch at the seek target; calls: {:?}",
        log.calls()
    );
}

#[test]
fn rapid_seek_drops_stale_delivery() {
    // The fetch at A is gated; while it hangs, we seek to B. The
    // generation bump must drop A's (late) delivery entirely — only
    // B's range may be read back.
    let data: Arc<Vec<u8>> = Arc::new((0..64 * 1024u32).map(|i| (i * 3) as u8).collect());
    let a_pos: u64 = 8 * 1024;
    let b_pos: u64 = 40 * 1024;
    let (fetch, gate, log) = gated_fetch(Arc::clone(&data), a_pos);
    let mut src = RemoteSource::with_readahead(4 * 1024, fetch);

    // Park the cursor in A's range; the prefetch fetches there — the
    // gated call. (With no player probing, this is the only fetch so
    // far: construction's offset-0 fetch is superseded by the seek
    // before it's ever issued.)
    src.seek(SeekFrom::Start(a_pos)).unwrap();
    assert!(
        log.wait_for_calls(1),
        "fetch at A must be issued: {:?}",
        log.calls()
    );
    assert_eq!(log.calls()[0].offset, a_pos);

    // Scrub to B while A's fetch is gated; release A afterwards.
    src.seek(SeekFrom::Start(b_pos)).unwrap();
    assert!(
        log.wait_for_calls(2),
        "fetch at B must be issued: {:?}",
        log.calls()
    );
    gate.send(()).unwrap();

    let got = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(got, data[b_pos as usize..b_pos as usize + 4 * 1024]);
    assert_eq!(log.last_call().unwrap().offset, b_pos);
    // The delivered bytes came from B's fetch only — A's late delivery
    // was dropped (its bytes don't appear before B's).
}

#[test]
fn len_is_temporal_and_end_seek_rejects_unknown() {
    let data: Arc<Vec<u8>> = Arc::new((0..32 * 1024u32).map(|i| i as u8).collect());
    let (fetch, gate, _log) = gated_fetch(Arc::clone(&data), 0);
    let mut src = RemoteSource::with_readahead(8 * 1024, fetch);

    // Before the first report: unknown length, End-seek rejected.
    assert!(wait_until(Duration::from_secs(2), || true));
    assert_eq!(src.len(), None);
    let err = src.seek(SeekFrom::End(-4)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    // Release: the length arrives with the first delivery.
    gate.send(()).unwrap();
    assert!(wait_until(Duration::from_secs(2), || src.len().is_some()));
    assert_eq!(src.len(), Some(data.len() as u64));

    // And seek-from-end works against the discovered length.
    let at = src.seek(SeekFrom::End(-10)).unwrap();
    assert_eq!(at, data.len() as u64 - 10);
    let tail = read_n(&mut src, 10).unwrap();
    assert_eq!(tail, data[data.len() - 10..]);
}

#[test]
fn lying_eof_is_retried_not_trusted() {
    // First fetch reports the full length but delivers a quarter and
    // "ends". The adapter must retry the missing range rather than
    // expose a premature EOF.
    let data: Arc<Vec<u8>> = Arc::new((0..64 * 1024u32).map(|i| (i * 11) as u8).collect());
    let expected = Arc::clone(&data);
    let quarter = data.len() / 4;
    let lied = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let fetch = move |offset: u64, max_len: usize, reply: cantode::ReplyHandle| {
        if offset == 0 && !lied.swap(true, std::sync::atomic::Ordering::SeqCst) {
            reply.set_total_len(Some(data.len() as u64));
            reply.push_chunk(data[..quarter].to_vec());
            reply.finish_eof(); // the lie
            return;
        }
        reply.set_total_len(Some(data.len() as u64));
        let end = (offset + max_len as u64).min(data.len() as u64);
        if offset < end {
            reply.push_chunk(data[offset as usize..end as usize].to_vec());
        }
        if end >= data.len() as u64 {
            reply.finish_eof();
        }
    };
    let mut src = RemoteSource::with_readahead(64 * 1024, fetch);

    let mut all = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = src.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
    }
    assert_eq!(all, *expected, "retry must recover the full resource");
}

#[test]
fn retry_budget_then_sticky_error_then_seek_recovery() {
    let data: Arc<Vec<u8>> = Arc::new((0..16 * 1024u32).map(|i| i as u8).collect());
    let failing = Arc::new(Mutex::new(true));
    let fetch = {
        let data = Arc::clone(&data);
        let failing = Arc::clone(&failing);
        move |offset: u64, max_len: usize, reply: cantode::ReplyHandle| {
            if *failing.lock().unwrap() {
                reply.finish_error("network down".into());
                return;
            }
            let end = (offset + max_len as u64).min(data.len() as u64);
            if offset < end {
                reply.push_chunk(data[offset as usize..end as usize].to_vec());
            }
            reply.finish_eof();
        }
    };
    let mut src = RemoteSource::with_readahead(16 * 1024, fetch);

    // Persistent failure: the retry budget runs out and reads error.
    assert!(
        wait_until(Duration::from_secs(3), || {
            src.read(&mut [0u8; 1]).is_err()
        }),
        "exhausted retries must surface as a read error"
    );

    // Recovery: seek clears the sticky error and the budget.
    *failing.lock().unwrap() = false;
    src.seek(SeekFrom::Start(0)).unwrap();
    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data);
}

#[test]
fn readahead_bounds_the_fetched_lead() {
    // Backpressure: the fetcher may run ahead of the reader by ~the
    // readahead (×2 worst case), never by the whole resource.
    let data: Arc<Vec<u8>> = Arc::new(vec![7u8; 1024 * 1024]);
    let log = FetchLog::default();
    let fetch = serving_fetch(Arc::clone(&data), 4 * 1024, log.clone());
    let mut src = RemoteSource::with_readahead(64 * 1024, fetch);

    let mut read_total = 0usize;
    for _ in 0..16 {
        let got = read_n(&mut src, 2048).unwrap();
        read_total += got.len();
        std::thread::sleep(Duration::from_millis(1));
    }
    // Give the prefetch thread a moment to top up to its gate.
    std::thread::sleep(Duration::from_millis(50));

    let served = log.bytes_served();
    let bound = read_total as u64 + 2 * 64 * 1024 + 8 * 1024;
    assert!(
        served <= bound,
        "fetcher ran too far ahead: served {served}, read {read_total}"
    );
    assert!(
        served < data.len() as u64,
        "resource must not be fully consumed"
    );
}

// ============================================================================
// B/O — device-free player behavior over RemoteSource
// ============================================================================

fn wav(seconds: f32) -> Arc<Vec<u8>> {
    Arc::new(common::make_sine_wav(common::WavSpec {
        seconds,
        ..Default::default()
    }))
}

struct Harness {
    _cx: PlayerContext,
    player: Player,
    capture: Arc<Mutex<common::CaptureState>>,
    events: mpsc::Receiver<PlayerEvent>,
}

fn harness_with(
    fetch: impl Fn(u64, usize, cantode::ReplyHandle) + Send + Sync + 'static,
) -> Harness {
    let cx = PlayerContext::new().unwrap();
    let (capture, factory) = capture_factory(false);
    let event_sink = Arc::new(ChannelEventSink::new(1024));
    let events = event_sink.subscribe();

    let player = Player::with_config(
        &cx,
        PlayerConfig::default()
            .audio_sink_factory(factory)
            .event_sink(Some(event_sink)),
    )
    .expect("player construction failed");

    let src: Box<dyn AudioSource> = Box::new(RemoteSource::with_readahead(16 * 1024, fetch));
    player.load(src).expect("load");

    Harness {
        _cx: cx,
        player,
        capture,
        events,
    }
}

#[test]
fn remote_source_plays_through_bit_exact() {
    let data = wav(2.0);
    let h = harness_with(serving_fetch(
        Arc::clone(&data),
        8 * 1024,
        FetchLog::default(),
    ));

    h.player.play().unwrap();
    assert!(wait_for_ended(&h.events, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(100));

    let captured = h.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured.len(), reference.len());
    assert_eq!(captured, reference, "captured PCM must be bit-exact");
}

#[test]
fn premature_close_is_retried_not_ended() {
    // The fetcher drops the connection a third of the way in (a failed
    // HTTP chunk) on the first pass; the retry serves the rest. The
    // player must reach the REAL end — this is the phantom-`Ended`
    // regression test for RemoteSource.
    let data = wav(3.0);
    let cut = data.len() / 3;
    let expected = Arc::clone(&data);
    let dropped_once = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let fetch = move |offset: u64, max_len: usize, reply: cantode::ReplyHandle| {
        reply.set_total_len(Some(data.len() as u64));
        if offset < cut as u64 && !dropped_once.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let end = (offset + max_len as u64).min(cut as u64);
            reply.push_chunk(data[offset as usize..end as usize].to_vec());
            reply.finish_eof(); // premature close
            return;
        }
        let end = (offset + max_len as u64).min(data.len() as u64);
        if offset < end {
            reply.push_chunk(data[offset as usize..end as usize].to_vec());
        }
        if end >= data.len() as u64 {
            reply.finish_eof();
        }
    };
    let h = harness_with(fetch);

    h.player.play().unwrap();
    assert!(
        wait_for_ended(&h.events, Duration::from_secs(10)),
        "must reach the real end"
    );
    assert_eq!(h.player.state(), PlayerState::Ended);
    // Ended near the 3s duration, not at the 1s cut.
    assert!(
        h.player.position() > Duration::from_millis(2800),
        "ended prematurely at {:?}",
        h.player.position()
    );

    let captured = h.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&expected);
    assert_eq!(captured.len(), reference.len());
    assert_eq!(captured, reference, "recovered audio must be bit-exact");
}

#[test]
fn stall_freezes_then_resumes_with_continuity() {
    let data = wav(3.0);
    let gate_at = 88_201u64; // ~0.5s in, well past the header
    let (fetch, gate, _log) = gated_fetch(Arc::clone(&data), gate_at);
    let h = harness_with(fetch);

    h.player.play().unwrap();
    let frozen = wait_for_quiet(Duration::from_secs(6), Duration::from_millis(300), || {
        h.player.position()
    })
    .expect("position must freeze while the fetch stalls");
    assert!(frozen > Duration::from_millis(300), "froze at {frozen:?}");
    assert_eq!(h.player.state(), PlayerState::Playing);

    gate.send(()).unwrap();
    assert!(wait_for_ended(&h.events, Duration::from_secs(10)));

    let captured = h.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured, reference, "post-stall audio must be continuous");
}

#[test]
fn persistent_fetch_error_stays_playing_silent() {
    let data = wav(2.0);
    let cut = 88_244usize; // ~0.5s
    let fetch = move |offset: u64, max_len: usize, reply: cantode::ReplyHandle| {
        reply.set_total_len(Some(data.len() as u64));
        if offset >= cut as u64 {
            reply.finish_error("network down".into());
            return;
        }
        let end = (offset + max_len as u64).min(cut as u64);
        reply.push_chunk(data[offset as usize..end as usize].to_vec());
        // The boundary fetch cannot complete: the network "dies" at the
        // cut, so fail it rather than hang it (the hang case is what
        // FETCH_DEADLINE bounds — too slow to test here).
        if end < offset + max_len as u64 {
            reply.finish_error("network down".into());
        }
    };
    let h = harness_with(fetch);

    h.player.play().unwrap();
    let frozen = wait_for_quiet(Duration::from_secs(8), Duration::from_millis(400), || {
        h.player.position()
    })
    .expect("position must freeze when fetches fail");
    assert!(
        frozen > Duration::from_millis(300) && frozen < Duration::from_secs(2),
        "froze at {frozen:?}, expected near the ~0.5s failure point"
    );
    assert_eq!(h.player.state(), PlayerState::Playing);
    assert!(
        !wait_for_ended(&h.events, Duration::from_millis(300)),
        "persistent fetch errors must not emit Ended"
    );
}
