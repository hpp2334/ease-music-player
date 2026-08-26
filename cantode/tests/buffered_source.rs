//! Tests for [`BufferedSource`] — cantode's windowed [`AudioSource`] over
//! a biz-implemented [`RemoteAudioSource`] session trait.
//!
//! The fake (`FakeRemote` below) maps the trait onto a RAM byte store and
//! scripts network manners: delivery gates (stalls), cut-short sessions
//! (lying EOF / premature close), scheduled failures, late length
//! reports, over-delivery. Because every trait method is non-blocking,
//! a "stall" is simply a delivery that hasn't happened yet — the fake
//! parks it on a gate in a spawned thread, never inside `request`.
//!
//! Coverage map:
//!
//! - **T0** (no player): one session per play-through (the headline);
//!   demand follows reads; seek inside the window touches no network;
//!   seek outside closes + reopens at the target; rapid scrub leaves
//!   only the final session serving; temporal `len()` and seek-from-end
//!   rejection while unknown; lying-EOF retry; retry budget → sticky
//!   error → seek recovery; watchdog reopen; over-delivery rejection;
//!   inline (synchronous) delivery from inside `request`; `Drop` closes.
//! - **B/O** (device-free player): play-through bit-exactness with
//!   exactly one session; the phantom-`Ended` fix (a premature close of
//!   a known-length resource is retried, not ended); stall
//!   freeze/resume with output continuity; persistent errors staying
//!   `Playing`-silent.

use std::io::{Read, Seek, SeekFrom};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use cantode::{
    AudioSource, BufferedSource, ChannelEventSink, Player, PlayerConfig, PlayerContext,
    PlayerEvent, PlayerState, Pushed, Readiness, RemoteAudioSource, StreamReply,
};

mod common;

use common::{capture_factory, reference_decode, wait_for_ended, wait_for_quiet, wait_until};

// ============================================================================
// FakeRemote — a scripted RemoteAudioSource over RAM
// ============================================================================

/// One trait invocation, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Open(u64),
    Request(usize),
    Close,
}

/// Log of trait invocations, shared between the boxed fake and the test.
#[derive(Clone, Default)]
struct SessionLog {
    calls: Arc<Mutex<Vec<Call>>>,
}

impl SessionLog {
    fn push(&self, c: Call) {
        self.calls.lock().unwrap().push(c);
    }
    fn all(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn opens(&self) -> Vec<u64> {
        self.all()
            .iter()
            .filter_map(|c| match c {
                Call::Open(o) => Some(*o),
                _ => None,
            })
            .collect()
    }
    fn closes(&self) -> usize {
        self.all()
            .iter()
            .filter(|c| matches!(c, Call::Close))
            .count()
    }
    fn requests(&self) -> Vec<usize> {
        self.all()
            .iter()
            .filter_map(|c| match c {
                Call::Request(w) => Some(*w),
                _ => None,
            })
            .collect()
    }
    fn wait_for_opens(&self, n: usize) -> bool {
        wait_until(Duration::from_secs(3), || self.opens().len() >= n)
    }
    /// Wait until the call log stops growing for `quiet`.
    fn wait_for_quiet(&self, quiet: Duration) -> bool {
        let mut last = self.all().len();
        let end = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            std::thread::sleep(quiet);
            let now = self.all().len();
            if now == last {
                return true;
            }
            last = now;
            if std::time::Instant::now() >= end {
                return false;
            }
        }
    }
}

/// A release-once latch all gated deliveries wait on (broadcast, so a
/// stall can block several superseded sessions at once).
#[derive(Default)]
struct Gate {
    released: Mutex<bool>,
    cv: std::sync::Condvar,
}

impl Gate {
    fn release(&self) {
        let mut r = self.released.lock().unwrap();
        *r = true;
        self.cv.notify_all();
    }
    fn wait_released(&self) {
        let mut r = self.released.lock().unwrap();
        while !*r {
            r = self.cv.wait(r).unwrap();
        }
    }
    /// Whether a request at `cursor` should block on this gate now.
    fn gated_now(&self, cursor: u64, gate_at: Option<u64>) -> bool {
        gate_at.is_some_and(|at| cursor >= at) && !*self.released.lock().unwrap()
    }
}

/// Script knobs + per-session serving state. Shared behind a `Mutex`
/// between the boxed fake and the test's handle.
#[derive(Default)]
struct FakeState {
    /// Next byte to serve (advances by *accepted* bytes only).
    cursor: u64,
    /// The live session's reply (set at `open`, cleared at `close`).
    reply: Option<StreamReply>,
    /// Delivery gate: a request whose cursor is at/after this offset
    /// blocks (in a spawned thread) until the gate is released.
    gate_at: Option<u64>,
    gate: Option<Arc<Gate>>,
    /// First session serves only up to this offset, then `finish_eof`
    /// short of the reported total (a lying EOF / premature close).
    /// Once only.
    cut_once: Option<u64>,
    cut_fired: bool,
    /// `finish_error` once the cursor reaches this offset.
    fail_from: Option<u64>,
    /// Report the total at the first delivery instead of at `open`
    /// (Content-Length arriving late).
    report_total_late: bool,
    total_reported: bool,
    /// Push granularity in bytes (0 = one push per request).
    chunk: usize,
    /// Over-deliver: pad the first push of each request by this many
    /// bytes beyond the granted demand.
    over_deliver: usize,
    /// Recorded `(pushed, accepted)` pairs.
    pushes: Vec<(usize, usize)>,
    /// Thread that ran `request`; threads that ran pushes (for the
    /// inline-delivery assertion).
    request_thread: Option<ThreadId>,
    push_threads: Vec<ThreadId>,
}

/// The boxed, shareable fake.
struct FakeRemote {
    data: Arc<Vec<u8>>,
    log: SessionLog,
    st: Arc<Mutex<FakeState>>,
}

/// The test-side handle (the fake itself is moved into the source).
#[derive(Clone)]
struct FakeHandle {
    log: SessionLog,
    st: Arc<Mutex<FakeState>>,
    gate: Option<Arc<Gate>>,
}

/// Build a fake pair over `data`. Knobs are set via the returned
/// builder before `finish` boxes the fake.
struct FakeBuilder {
    data: Arc<Vec<u8>>,
    log: SessionLog,
    st: Arc<Mutex<FakeState>>,
    gate: Option<Arc<Gate>>,
}

fn fake(data: Arc<Vec<u8>>) -> FakeBuilder {
    FakeBuilder {
        log: SessionLog::default(),
        st: Arc::new(Mutex::new(FakeState {
            chunk: 8 * 1024,
            ..Default::default()
        })),
        data,
        gate: None,
    }
}

impl FakeBuilder {
    /// Arm the delivery gate at `at`.
    fn gate_at(mut self, at: u64) -> Self {
        let gate = Arc::new(Gate::default());
        self.gate = Some(Arc::clone(&gate));
        {
            let mut st = self.st.lock().unwrap();
            st.gate_at = Some(at);
            st.gate = Some(gate);
        }
        self
    }
    fn cut_once(self, at: u64) -> Self {
        self.st.lock().unwrap().cut_once = Some(at);
        self
    }
    fn fail_from(self, at: u64) -> Self {
        self.st.lock().unwrap().fail_from = Some(at);
        self
    }
    fn report_total_late(self) -> Self {
        self.st.lock().unwrap().report_total_late = true;
        self
    }
    fn chunk(self, n: usize) -> Self {
        self.st.lock().unwrap().chunk = n;
        self
    }
    fn over_deliver(self, n: usize) -> Self {
        self.st.lock().unwrap().over_deliver = n;
        self
    }
    /// Box the fake and return it with the test-side handle.
    fn finish(self) -> (Box<FakeRemote>, FakeHandle) {
        let boxed = Box::new(FakeRemote {
            data: self.data,
            log: self.log.clone(),
            st: Arc::clone(&self.st),
        });
        let handle = FakeHandle {
            log: self.log,
            st: self.st,
            gate: self.gate,
        };
        (boxed, handle)
    }
}

impl FakeHandle {
    fn opens(&self) -> Vec<u64> {
        self.log.opens()
    }
    fn closes(&self) -> usize {
        self.log.closes()
    }
    fn requests(&self) -> Vec<usize> {
        self.log.requests()
    }
    fn pushes(&self) -> Vec<(usize, usize)> {
        self.st.lock().unwrap().pushes.clone()
    }
    fn request_and_push_threads(&self) -> (Option<ThreadId>, Vec<ThreadId>) {
        let st = self.st.lock().unwrap();
        (st.request_thread, st.push_threads.clone())
    }
    fn release_gate(&self) {
        if let Some(gate) = &self.gate {
            gate.release();
        }
    }
    fn clear_fail(&self) {
        self.st.lock().unwrap().fail_from = None;
    }
}

impl FakeRemote {
    /// Serve up to `want` bytes from the cursor through `reply`,
    /// advancing the cursor by accepted bytes only. A partial acceptance
    /// (window full) defers the remainder to the next request — exactly
    /// the "keep the tail" contract.
    fn serve(data: &[u8], st: &mut FakeState, reply: &StreamReply, want: usize) {
        if st.report_total_late && !st.total_reported {
            st.total_reported = true;
            reply.set_total_len(Some(data.len() as u64));
        }
        let cursor = st.cursor;
        if st.fail_from.is_some_and(|f| cursor >= f) {
            reply.finish_error("scripted failure".into());
            return;
        }
        let mut end = (cursor + want as u64).min(data.len() as u64);
        if let Some(cut) = st.cut_once
            && !st.cut_fired
            && cursor < cut
        {
            end = end.min(cut);
        }
        let mut at = cursor;
        while at < end {
            let base = (end - at) as usize;
            let mut pushed_len = if st.chunk == 0 || st.over_deliver > 0 {
                base
            } else {
                base.min(st.chunk)
            };
            if st.over_deliver > 0 && at == cursor {
                pushed_len += st.over_deliver;
            }
            pushed_len = pushed_len.min(data.len() - at as usize);
            let bytes = data[at as usize..at as usize + pushed_len].to_vec();
            match reply.push(bytes) {
                Pushed::Superseded => break,
                Pushed::Accepted(n) => {
                    st.pushes.push((pushed_len, n));
                    st.push_threads.push(std::thread::current().id());
                    at += n as u64;
                    if n < pushed_len {
                        // Rejected tail: keep it (the cursor stayed put);
                        // retry against future demand.
                        break;
                    }
                }
            }
        }
        st.cursor = at;
        if st.cut_once == Some(at) && !st.cut_fired {
            st.cut_fired = true;
            reply.finish_eof(); // short of the reported total — the lie
            return;
        }
        if at >= data.len() as u64 {
            reply.finish_eof();
        }
    }
}

impl RemoteAudioSource for FakeRemote {
    fn open(&self, offset: u64, reply: StreamReply) {
        self.log.push(Call::Open(offset));
        let mut st = self.st.lock().unwrap();
        st.cursor = offset;
        if !st.report_total_late && st.total_reported {
            // A later session of the same resource: the length is known.
        }
        if !st.report_total_late {
            st.total_reported = true;
            reply.set_total_len(Some(self.data.len() as u64));
        }
        st.reply = Some(reply);
    }

    fn request(&self, want: usize) {
        self.log.push(Call::Request(want));
        let (reply, gated) = {
            let mut st = self.st.lock().unwrap();
            st.request_thread = Some(std::thread::current().id());
            let reply = st.reply.clone();
            let gated = st
                .gate
                .clone()
                .filter(|g| g.gated_now(st.cursor, st.gate_at));
            (reply, gated)
        };
        let Some(reply) = reply else { return };
        if let Some(gate) = gated {
            // Park only the delivery, never the session thread — the
            // contract the watchdog depends on.
            let st = Arc::clone(&self.st);
            let data = Arc::clone(&self.data);
            std::thread::spawn(move || {
                gate.wait_released();
                let mut st = st.lock().unwrap();
                FakeRemote::serve(&data, &mut st, &reply, want);
            });
        } else {
            // Inline delivery: re-enters cantode synchronously from
            // inside `request` (allowed — cantode never holds its lock
            // across trait calls).
            let mut st = self.st.lock().unwrap();
            FakeRemote::serve(&self.data, &mut st, &reply, want);
        }
    }

    fn close(&self) {
        self.log.push(Call::Close);
        self.st.lock().unwrap().reply = None;
    }
}

// ============================================================================
// T0 — adapter mechanics, no player
// ============================================================================

fn read_n(src: &mut BufferedSource, n: usize) -> std::io::Result<Vec<u8>> {
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

fn pattern(n: usize, f: impl Fn(u32) -> u8) -> Arc<Vec<u8>> {
    Arc::new((0..n as u32).map(f).collect())
}

#[test]
fn one_session_serves_whole_resource() {
    // The headline: a full play-through opens exactly ONE session — no
    // re-opens at readahead window edges — and demand (request sizes)
    // stays within the readahead while summing to at least the resource.
    let data = pattern(100_000, |i| i as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(16 * 1024, fake);

    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data, "full read must reconstruct the file");

    assert_eq!(h.opens(), vec![0], "exactly one session: {:?}", h.opens());
    assert_eq!(h.closes(), 0, "drop closes; nothing before it");

    let reqs = h.requests();
    assert!(!reqs.is_empty());
    assert!(
        reqs.iter().all(|w| *w <= 16 * 1024),
        "every demand within the readahead: {reqs:?}"
    );
    assert!(
        reqs.iter().sum::<usize>() >= data.len(),
        "demands must cover the resource: {reqs:?}"
    );
}

#[test]
fn demand_rises_as_reads_drain() {
    // Window full ⇒ no requests; after draining, more requests arrive.
    let data = pattern(64 * 1024, |i| (i * 7) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(16 * 1024, fake);

    let _ = read_n(&mut src, 8 * 1024).unwrap();
    assert!(
        h.log.wait_for_quiet(Duration::from_millis(80)),
        "requests must settle"
    );
    let settled = h.requests().len();
    assert!(settled >= 1);

    let _ = read_n(&mut src, 8 * 1024).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || h.requests().len() > settled),
        "demand must rise after reads drain the window"
    );
}

#[test]
fn seek_inside_window_hits_no_network() {
    // Readahead ≥ resource: one session covers everything, and its EOF
    // means no top-ups — so any second open would have to come from the
    // seek. An in-window seek must not trigger one.
    let data = pattern(32 * 1024, |i| (i * 7) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(32 * 1024, fake);

    let first = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(first, data[..4 * 1024]);
    assert!(
        wait_until(Duration::from_secs(2), || h.opens() == vec![0]),
        "initial session must open: {:?}",
        h.opens()
    );

    src.seek(SeekFrom::Start(1024)).unwrap();
    let again = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(again, data[1024..5 * 1024]);
    assert_eq!(h.opens(), vec![0], "in-window seek must not open a session");
    assert_eq!(h.closes(), 0, "in-window seek must not close the session");
}

#[test]
fn seek_outside_window_closes_and_reopens_at_target() {
    let data = pattern(128 * 1024, |i| (i * 13) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(8 * 1024, fake);

    read_n(&mut src, 1024).unwrap();
    src.seek(SeekFrom::Start(60_000)).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || h.opens().contains(&60_000)),
        "expected a session at the seek target: {:?}",
        h.opens()
    );
    let got = read_n(&mut src, 4_000).unwrap();
    assert_eq!(got, data[60_000..64_000]);
    assert!(
        h.closes() >= 1,
        "the abandoned session must be closed: {:?}",
        h.log.all()
    );
}

#[test]
fn rapid_seek_leaves_only_final_session_serving() {
    // The session at A is gated; while its delivery hangs, we scrub to
    // B. A's late delivery must be dropped entirely (generation guard)
    // — only B's range may be read back.
    let data = pattern(64 * 1024, |i| (i * 3) as u8);
    let a_pos: u64 = 8 * 1024;
    let b_pos: u64 = 40 * 1024;
    let (fake, h) = fake(Arc::clone(&data)).gate_at(a_pos).finish();
    let mut src = BufferedSource::with_readahead(4 * 1024, fake);

    src.seek(SeekFrom::Start(a_pos)).unwrap();
    assert!(
        h.log.wait_for_opens(1),
        "a session must open: {:?}",
        h.opens()
    );
    // (Whether the very first session at 0 raced the seek is irrelevant;
    // the gated session at A exists once an Open{a_pos} is logged.)
    assert!(
        wait_until(Duration::from_secs(2), || h.opens().contains(&a_pos)),
        "session at A: {:?}",
        h.opens()
    );

    src.seek(SeekFrom::Start(b_pos)).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || h.opens().contains(&b_pos)),
        "session at B: {:?}",
        h.opens()
    );
    h.release_gate(); // A's delivery fires — into a superseded session

    let got = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(got, data[b_pos as usize..b_pos as usize + 4 * 1024]);
    assert_eq!(*h.opens().last().unwrap(), b_pos);
}

#[test]
fn len_is_temporal_and_end_seek_rejects_unknown() {
    let data = pattern(32 * 1024, |i| i as u8);
    let (fake, h) = fake(Arc::clone(&data))
        .gate_at(0)
        .report_total_late()
        .finish();
    let mut src = BufferedSource::with_readahead(8 * 1024, fake);

    // Before the first report: unknown length, End-seek rejected.
    assert_eq!(src.len(), None);
    let err = src.seek(SeekFrom::End(-4)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        src.readiness(),
        Readiness::NeedsData,
        "gated session with an empty window must report NeedsData"
    );

    // Release: the length arrives with the first delivery.
    h.release_gate();
    assert!(wait_until(Duration::from_secs(2), || src.len().is_some()));
    assert_eq!(src.len(), Some(data.len() as u64));
    assert_eq!(
        src.readiness(),
        Readiness::Ready,
        "delivered data must report Ready"
    );

    // And seek-from-end works against the discovered length.
    let at = src.seek(SeekFrom::End(-10)).unwrap();
    assert_eq!(at, data.len() as u64 - 10);
    let tail = read_n(&mut src, 10).unwrap();
    assert_eq!(tail, data[data.len() - 10..]);
}

#[test]
fn lying_eof_is_retried_not_trusted() {
    // The first session reports the full length but ends a quarter in.
    // The adapter must retry the missing range rather than expose a
    // premature EOF.
    let data = pattern(64 * 1024, |i| (i * 11) as u8);
    let quarter = data.len() / 4;
    let (fake, _h) = fake(Arc::clone(&data)).cut_once(quarter as u64).finish();
    let mut src = BufferedSource::with_readahead(64 * 1024, fake);

    let mut all = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = src.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
    }
    assert_eq!(all, *data, "retry must recover the full resource");
}

#[test]
fn retry_budget_then_sticky_error_then_seek_recovery() {
    let data = pattern(16 * 1024, |i| i as u8);
    let (fake, h) = fake(Arc::clone(&data)).fail_from(0).finish();
    let mut src = BufferedSource::with_readahead(16 * 1024, fake);

    // Persistent failure: the retry budget runs out and reads error.
    assert!(
        wait_until(Duration::from_secs(3), || {
            src.read(&mut [0u8; 1]).is_err()
        }),
        "exhausted retries must surface as a read error"
    );

    // Recovery: seek clears the sticky error and the budget.
    h.clear_fail();
    src.seek(SeekFrom::Start(0)).unwrap();
    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data);
}

#[test]
fn watchdog_reopens_stalled_session() {
    // A session with outstanding demand that never delivers is closed
    // (aborting the transport) and the range retried at the window end.
    let data = pattern(128 * 1024, |i| (i * 5) as u8);
    let (fake, h) = fake(Arc::clone(&data)).gate_at(8 * 1024).finish();
    let mut src =
        BufferedSource::with_readahead(16 * 1024, fake).watchdog(Duration::from_millis(150));

    // Session 1 delivers the first window (crossing the gate), then its
    // next request parks on the gate — no progress.
    let _ = read_n(&mut src, 1024).unwrap();
    assert!(
        h.log.wait_for_opens(2),
        "the watchdog must reopen: {:?}",
        h.opens()
    );
    assert_eq!(h.opens()[1], 16 * 1024, "reopen at the window end");
    assert!(h.closes() >= 1, "the stalled session must be closed");

    // The stalled delivery fires late — into a superseded session.
    h.release_gate();
    src.seek(SeekFrom::Start(0)).unwrap();
    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data, "recovery must reconstruct the resource");
}

#[test]
fn watchdog_exhausts_to_sticky_error() {
    let data = pattern(16 * 1024, |i| i as u8);
    let (fake, h) = fake(Arc::clone(&data)).gate_at(0).finish();
    let mut src =
        BufferedSource::with_readahead(16 * 1024, fake).watchdog(Duration::from_millis(100));

    // Never released: the budget runs out and reads fail.
    assert!(
        wait_until(Duration::from_secs(3), || {
            src.read(&mut [0u8; 1]).is_err()
        }),
        "a permanently stalled source must surface a read error"
    );
    let _ = h; // gate deliberately never released
}

#[test]
fn over_delivery_is_rejected_not_truncated() {
    // A push beyond the granted demand reports a partial acceptance and
    // the tail is re-offered later — never silently dropped or counted.
    let data = pattern(100 * 1024, |i| (i * 17) as u8);
    let (fake, h) = fake(Arc::clone(&data)).over_deliver(4096).finish();
    let mut src = BufferedSource::with_readahead(16 * 1024, fake);

    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data, "over-delivery must not corrupt the stream");

    let pushes = h.pushes();
    assert!(
        pushes.iter().any(|(p, a)| a < p),
        "expected a rejected tail: {pushes:?}"
    );
    let accepted: usize = pushes.iter().map(|(_, a)| a).sum();
    assert_eq!(accepted, data.len(), "accepted bytes must be exact");
}

#[test]
fn inline_delivery_from_request_does_not_deadlock() {
    // The fake delivers synchronously inside `request` (the session
    // thread) — cantode must never hold its state lock across a trait
    // call, so this re-entry is legal. Verified by thread identity plus
    // the test completing at all.
    let data = pattern(32 * 1024, |i| (i * 7) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(8 * 1024, fake);

    let got = read_n(&mut src, data.len()).unwrap();
    assert_eq!(got, *data);

    let (request_thread, push_threads) = h.request_and_push_threads();
    let request_thread = request_thread.expect("request ran");
    assert!(
        push_threads.iter().all(|t| *t == request_thread),
        "deliveries must have run inline in request"
    );
}

#[test]
fn drop_closes_the_session() {
    let data = pattern(16 * 1024, |i| i as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(16 * 1024, fake);
    read_n(&mut src, 1024).unwrap();
    assert_eq!(h.closes(), 0);

    drop(src);
    assert!(
        wait_until(Duration::from_secs(2), || h.closes() >= 1),
        "drop must close the live session"
    );
}

// ============================================================================
// B/O — device-free player behavior over BufferedSource
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

fn harness_with(fake: Box<FakeRemote>, readahead: usize) -> Harness {
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

    let src: Box<dyn AudioSource> = Box::new(BufferedSource::with_readahead(readahead, fake));
    player.load(src).expect("load");

    Harness {
        _cx: cx,
        player,
        capture,
        events,
    }
}

#[test]
fn plays_through_bit_exact_with_one_session() {
    let data = wav(2.0);
    let (fake, h) = fake(Arc::clone(&data)).chunk(8 * 1024).finish();
    let harness = harness_with(fake, 16 * 1024);

    harness.player.play().unwrap();
    assert!(wait_for_ended(&harness.events, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(100));

    let captured = harness.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured.len(), reference.len());
    assert_eq!(captured, reference, "captured PCM must be bit-exact");

    assert_eq!(h.opens(), vec![0], "one session per play-through");
    assert_eq!(h.closes(), 0, "no session churn while playing");
}

#[test]
fn premature_close_is_retried_not_ended() {
    // The session dies a third of the way in (a dropped connection) on
    // the first pass; the retry streams the rest. The player must reach
    // the REAL end — the phantom-`Ended` regression test.
    let data = wav(3.0);
    let cut = data.len() / 3;
    let (fake, _h) = fake(Arc::clone(&data)).cut_once(cut as u64).finish();
    let harness = harness_with(fake, 16 * 1024);

    harness.player.play().unwrap();
    assert!(
        wait_for_ended(&harness.events, Duration::from_secs(10)),
        "must reach the real end"
    );
    assert_eq!(harness.player.state(), PlayerState::Ended);
    // Ended near the 3s duration, not at the 1s cut.
    assert!(
        harness.player.position() > Duration::from_millis(2800),
        "ended prematurely at {:?}",
        harness.player.position()
    );

    let captured = harness.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured.len(), reference.len());
    assert_eq!(captured, reference, "recovered audio must be bit-exact");
}

#[test]
fn stall_freezes_then_resumes_with_continuity() {
    let data = wav(3.0);
    let gate_at = 88_201u64; // ~0.5s in, well past the header
    let (fake, h) = fake(Arc::clone(&data)).gate_at(gate_at).finish();
    let harness = harness_with(fake, 16 * 1024);

    harness.player.play().unwrap();
    let frozen = wait_for_quiet(Duration::from_secs(6), Duration::from_millis(300), || {
        harness.player.position()
    })
    .expect("position must freeze while the session stalls");
    assert!(frozen > Duration::from_millis(300), "froze at {frozen:?}");
    assert_eq!(harness.player.state(), PlayerState::Playing);

    h.release_gate();
    assert!(wait_for_ended(&harness.events, Duration::from_secs(10)));

    let captured = harness.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured, reference, "post-stall audio must be continuous");
}

#[test]
fn persistent_error_stays_playing_silent() {
    let data = wav(2.0);
    let cut = 88_244usize; // ~0.5s
    let (fake, _h) = fake(Arc::clone(&data)).fail_from(cut as u64).finish();
    let harness = harness_with(fake, 16 * 1024);

    harness.player.play().unwrap();
    let frozen = wait_for_quiet(Duration::from_secs(8), Duration::from_millis(400), || {
        harness.player.position()
    })
    .expect("position must freeze when sessions fail");
    assert!(
        frozen > Duration::from_millis(300) && frozen < Duration::from_secs(2),
        "froze at {frozen:?}, expected near the ~0.5s failure point"
    );
    assert_eq!(harness.player.state(), PlayerState::Playing);
    assert!(
        !wait_for_ended(&harness.events, Duration::from_millis(300)),
        "persistent session errors must not emit Ended"
    );
}
