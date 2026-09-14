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
//!   demand follows reads; seek inside the window touches no network
//!   (and keeps the live session); seek outside closes + reopens at the
//!   target, parking the abandoned window so a seek back restores it
//!   with no network (the isomp4-prologue shape); rapid scrub leaves
//!   only the final session serving; temporal `len()` and seek-from-end
//!   rejection while unknown; lying-EOF retry; retry budget → sticky
//!   error → seek recovery; delivered progress resetting the budget;
//!   watchdog reopen; over-delivery rejection;
//!   inline (synchronous) delivery from inside `request`; `Drop` closes.
//! - **B/O** (device-free player): play-through bit-exactness with
//!   exactly one session; the phantom-`Ended` fix (a premature close of
//!   a known-length resource is retried, not ended); stall
//!   freeze/resume with output continuity; persistent errors staying
//!   `Playing`-silent; min-buffer gating — the autoplay startup
//!   prebuffer, refill waiting for the cushion, the thin-readahead
//!   park, and the end-of-stream resume escape.

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
    /// Delivery ceiling: serve no bytes at/above this offset until the
    /// test raises it (`FakeHandle::raise_deliver_until`). Unlike the
    /// release-once gate — which parks whole *requests* and lets an
    /// in-flight span leap the gate offset — the ceiling clamps every
    /// push, so it scripts exactly "the network has delivered [0, cap)"
    /// regardless of demand sizes. `None` = unlimited.
    deliver_until: Option<u64>,
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
    /// Cap delivery below `cap` bytes (see [`FakeState::deliver_until`]);
    /// raise it from the handle with
    /// [`FakeHandle::raise_deliver_until`].
    fn deliver_until(self, cap: u64) -> Self {
        self.st.lock().unwrap().deliver_until = Some(cap);
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
    /// Raise the delivery ceiling (never lowers it).
    fn raise_deliver_until(&self, cap: u64) {
        let mut st = self.st.lock().unwrap();
        if st.deliver_until.is_none_or(|c| cap > c) {
            st.deliver_until = Some(cap);
        }
    }
    fn clear_fail(&self) {
        self.st.lock().unwrap().fail_from = None;
    }
}

/// Outcome of one [`FakeRemote::serve`] pass, consumed by the
/// delivery-ceiling chaining in [`deliver_aware`].
enum Served {
    /// Nothing more to do now: the demand was met, EOF fired, or the
    /// adapter's window rejected the tail (future demand re-offers it).
    Done,
    /// The session was superseded — stop touching its reply.
    Superseded,
    /// Stopped at the delivery ceiling with `remaining` bytes of the
    /// demand still owed; a watcher must re-offer them once the
    /// ceiling rises past the cursor.
    Capped { remaining: usize },
}

impl FakeRemote {
    /// Serve up to `want` bytes from the cursor through `reply`,
    /// advancing the cursor by accepted bytes only. A partial acceptance
    /// (window full) defers the remainder to the next request — exactly
    /// the "keep the tail" contract.
    fn serve(data: &[u8], st: &mut FakeState, reply: &StreamReply, want: usize) -> Served {
        if st.report_total_late && !st.total_reported {
            st.total_reported = true;
            reply.set_total_len(Some(data.len() as u64));
        }
        let cursor = st.cursor;
        if st.fail_from.is_some_and(|f| cursor >= f) {
            reply.finish_error("scripted failure".into());
            return Served::Done;
        }
        let mut end = (cursor + want as u64).min(data.len() as u64);
        if let Some(cap) = st.deliver_until {
            end = end.min(cap);
        }
        if let Some(cut) = st.cut_once
            && !st.cut_fired
            && cursor < cut
        {
            end = end.min(cut);
        }
        let mut at = cursor;
        let mut superseded = false;
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
                Pushed::Superseded => {
                    superseded = true;
                    break;
                }
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
            return Served::Done;
        }
        if at >= data.len() as u64 {
            reply.finish_eof();
            return Served::Done;
        }
        if superseded {
            return Served::Superseded;
        }
        let delivered = (at - cursor) as usize;
        if delivered < want && st.deliver_until == Some(at) && (at as usize) < data.len() {
            return Served::Capped {
                remaining: want - delivered,
            };
        }
        Served::Done
    }
}

/// Serve `want`, parking (off the caller's thread) whenever the
/// delivery ceiling pins the cursor, and re-offering the remainder as
/// the ceiling rises. One thread per capped span; no thread at all
/// without a ceiling.
fn deliver_aware(
    data: Arc<Vec<u8>>,
    st: Arc<Mutex<FakeState>>,
    reply: StreamReply,
    mut want: usize,
) {
    loop {
        let outcome = {
            let mut s = st.lock().unwrap();
            FakeRemote::serve(&data, &mut s, &reply, want)
        };
        match outcome {
            Served::Capped { remaining } => {
                want = remaining;
                wait_ceiling_rises(&st);
            }
            Served::Done | Served::Superseded => return,
        }
    }
}

/// Poll until the cursor sits below the delivery ceiling (or there is
/// none). Polling, not a condvar: the ceiling may rise repeatedly and
/// the park happens on disposable test threads.
fn wait_ceiling_rises(st: &Mutex<FakeState>) {
    loop {
        let open = {
            let st = st.lock().unwrap();
            st.deliver_until.is_none_or(|cap| st.cursor < cap)
        };
        if open {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
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
                deliver_aware(data, st, reply, want);
            });
        } else {
            // Inline delivery: re-enters cantode synchronously from
            // inside `request` (allowed — cantode never holds its lock
            // across trait calls). A capped span hands the remainder to
            // a watcher thread instead.
            let outcome = {
                let mut st = self.st.lock().unwrap();
                FakeRemote::serve(&self.data, &mut st, &reply, want)
            };
            if let Served::Capped { remaining } = outcome {
                let st = Arc::clone(&self.st);
                let data = Arc::clone(&self.data);
                std::thread::spawn(move || deliver_aware(data, st, reply, remaining));
            }
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
fn buffered_range_tracks_the_window() {
    // `buffered_range` is the embedder-facing projection of the window:
    // start/end in absolute bytes plus the reported total. It fills to
    // cursor+readahead, keeps its start below the retention cap, and
    // resets with the window on an out-of-window seek.
    let data = pattern(128 * 1024, |i| (i * 5) as u8);
    let (fake, _h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(8 * 1024, fake);

    // The construction-time session fills the window at cursor 0.
    assert!(
        wait_until(Duration::from_secs(2), || {
            src.buffered_range()
                .is_some_and(|r| r.start == 0 && r.end == 8 * 1024)
        }),
        "window must fill to readahead: {:?}",
        src.buffered_range()
    );
    assert_eq!(src.buffered_range().unwrap().total, Some(data.len() as u64));

    // Reads advance the cursor; the window refills to cursor+readahead.
    read_n(&mut src, 4 * 1024).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || {
            src.buffered_range().is_some_and(|r| r.end == 12 * 1024)
        }),
        "window must refill to cursor+readahead: {:?}",
        src.buffered_range()
    );
    assert_eq!(
        src.buffered_range().unwrap().start,
        0,
        "no consumed-prefix eviction below the retention cap"
    );

    // An out-of-window seek resets the window to the target.
    src.seek(SeekFrom::Start(60_000)).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || {
            src.buffered_range()
                .is_some_and(|r| r.start == 60_000 && r.end == 60_000 + 8 * 1024)
        }),
        "window must restart at the seek target: {:?}",
        src.buffered_range()
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
fn in_window_seek_keeps_live_session() {
    // The module doc's contract, pinned for a LIVE session
    // (`seek_inside_window_hits_no_network` above covers one that has
    // already delivered to EOF): mid-stream, with the session still
    // open, an in-window seek is a cursor move — no Close, no new Open
    // — and the data read back is correct. Demuxers with
    // non-sequential sample layouts (isomp4 `next_packet` on
    // interleaved chunks) seek like this constantly; a miss inside the
    // readahead window must not cost an HTTP request.
    let data = pattern(64 * 1024, |i| (i * 7) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(16 * 1024, fake);

    let first = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(first, data[..4 * 1024]);
    assert_eq!(h.opens(), vec![0]);

    // Backward, still inside the window.
    src.seek(SeekFrom::Start(1024)).unwrap();
    let again = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(again, data[1024..5 * 1024]);
    assert_eq!(h.opens(), vec![0], "in-window seek must not open a session");
    assert_eq!(h.closes(), 0, "in-window seek must not close the session");
}

#[test]
fn seek_back_into_abandoned_window_restores_without_network() {
    // An out-of-window seek parks the abandoned window in the retention
    // slot; a seek back into it restores the bytes with no session.
    // This is symphonia's isomp4 prologue shape exactly: skip forward
    // past `mdat` (a tail probe), then return to byte 0 for the second
    // atom scan — the header region must not be re-fetched.
    let data = pattern(128 * 1024, |i| (i * 11) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(32 * 1024, fake);

    // Window [0, 36 KiB): the construction session (32 KiB) plus the
    // 4 KiB top-up the first read's drain grants. Wait for it to fill
    // — a seek racing the delivery would stash a partial span.
    let _ = read_n(&mut src, 4 * 1024).unwrap();
    assert_eq!(h.opens(), vec![0]);
    assert!(
        wait_until(Duration::from_secs(2), || {
            src.buffered_range().is_some_and(|r| r.end == 36 * 1024)
        }),
        "window must fill to cursor+readahead: {:?}",
        src.buffered_range()
    );

    // Skip forward, far past the window: one close, one new session.
    src.seek(SeekFrom::Start(96 * 1024)).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || h.opens() == vec![0, 96 * 1024]),
        "expected sessions [0, 96 KiB]: {:?}",
        h.opens()
    );
    assert!(h.closes() >= 1, "the abandoned session must be closed");
    let got = read_n(&mut src, 1024).unwrap();
    assert_eq!(got, data[96 * 1024..96 * 1024 + 1024]);

    // Return to the header region: served from the retained window.
    src.seek(SeekFrom::Start(0)).unwrap();
    assert!(
        h.log.wait_for_quiet(Duration::from_millis(80)),
        "no session work expected after the restore: {:?}",
        h.log.all()
    );
    assert_eq!(
        h.opens(),
        vec![0, 96 * 1024],
        "the restore must not open a session"
    );
    let got = read_n(&mut src, 8 * 1024).unwrap();
    assert_eq!(got, data[..8 * 1024], "restored bytes must be correct");

    // Reads past the restored span resume with ONE continuation
    // session at the window end — never a re-fetch of byte 0.
    assert!(
        wait_until(Duration::from_secs(2), || h.opens().len() == 3),
        "expected the continuation session: {:?}",
        h.opens()
    );
    assert_eq!(h.opens(), vec![0, 96 * 1024, 36 * 1024]);
}

#[test]
fn ping_pong_seeks_swap_the_two_retained_spans() {
    // After a restore, the outgoing window takes the retention slot, so
    // alternating between two regions settles into two fetches: every
    // return is a swap, not a re-fetch (plus at most one continuation
    // per region as reads drain its window).
    let data = pattern(128 * 1024, |i| (i * 17) as u8);
    let (fake, h) = fake(Arc::clone(&data)).finish();
    let mut src = BufferedSource::with_readahead(32 * 1024, fake);

    let _ = read_n(&mut src, 4 * 1024).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || {
            src.buffered_range().is_some_and(|r| r.end == 36 * 1024)
        }),
        "window must fill before the first bounce: {:?}",
        src.buffered_range()
    );
    src.seek(SeekFrom::Start(96 * 1024)).unwrap();
    assert!(
        wait_until(Duration::from_secs(2), || h.opens() == vec![0, 96 * 1024]),
        "two sessions so far: {:?}",
        h.opens()
    );
    let _ = read_n(&mut src, 1024).unwrap();

    // Bounce A ↔ B several times, reading a little each time.
    for (pos, len) in [
        (2 * 1024u64, 1024usize),
        (97 * 1024, 1024),
        (4 * 1024, 1024),
    ] {
        src.seek(SeekFrom::Start(pos)).unwrap();
        let got = read_n(&mut src, len).unwrap();
        assert_eq!(
            got,
            data[pos as usize..pos as usize + len],
            "bounce read at {pos}"
        );
    }

    assert!(
        h.log.wait_for_quiet(Duration::from_millis(80)),
        "seeks must settle: {:?}",
        h.log.all()
    );
    assert!(
        h.opens().len() <= 4,
        "ping-pong must reuse the two spans: {:?}",
        h.opens()
    );
    assert_eq!(
        h.opens().iter().filter(|o| **o == 0).count(),
        1,
        "byte 0 fetched exactly once: {:?}",
        h.opens()
    );
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
fn delivered_progress_resets_the_retry_budget() {
    // The progress reward, discriminated: two watchdog strikes first
    // (gated delivery, nothing accepted), then a good spell streaming
    // ≥ 1 MiB on the recovered session, then an armed failure storm.
    // The storm must get the FULL budget — the live session's strike
    // plus three reopen strikes before sticky — instead of inheriting
    // the earlier strikes (which would stick almost immediately).
    let data = pattern(2 * 1024 * 1024, |i| (i * 7) as u8);
    let (fake, h) = fake(Arc::clone(&data)).gate_at(0).finish();
    let mut src =
        BufferedSource::with_readahead(64 * 1024, fake).watchdog(Duration::from_millis(150));

    // Strikes: the construction session and its watchdog reopens park on
    // the gate. Open #1 = construction; #2/#3 = watchdog reopens ⇒ two
    // strikes banked before the good spell.
    assert!(
        h.log.wait_for_opens(3),
        "the watchdog must reopen twice: {:?}",
        h.opens()
    );
    h.release_gate();

    // The recovered session streams past the reward threshold.
    let want = 1024 * 1024 + 64 * 1024;
    let got = read_n(&mut src, want).unwrap();
    assert_eq!(got, data[..want], "the recovered stream must be exact");
    let opens_after_reward = h.opens().len();

    // Arm failures at the fake's cursor and drain the window: opens
    // repeat at the window end until the budget dies.
    let fail_at = h.st.lock().unwrap().cursor;
    h.st.lock().unwrap().fail_from = Some(fail_at);
    let sticky = wait_until(Duration::from_secs(3), || {
        src.read(&mut [0u8; 4096]).is_err()
    });
    assert!(sticky, "the failure storm must end sticky");

    let storm_opens = h.opens().len() - opens_after_reward;
    // The storm's FIRST strike is invisible in the open log: it fails a
    // `request` on the live session (no new open). A rewarded budget
    // therefore logs three opens (reopens at the window end: strikes
    // 2, 3, sticky on 4); a budget still carrying the two pre-reward
    // strikes would log exactly one before going sticky.
    assert!(
        storm_opens >= 3,
        "a rewarded budget must survive three reopen strikes after the live-session strike, got {storm_opens}: {:?}",
        h.opens()
    );
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
    harness_with_config(fake, readahead, Duration::ZERO, false)
}

/// The full harness: a `min_buffer_duration` for the refill gate and a
/// choice of `load` vs `load_and_play` (the latter exercises the
/// startup prebuffer morph). The legacy [`harness_with`] pins the
/// zero-threshold mechanics; the min-buffer tests come through here.
fn harness_with_config(
    fake: Box<FakeRemote>,
    readahead: usize,
    min_buffer: Duration,
    autoplay: bool,
) -> Harness {
    let cx = PlayerContext::new().unwrap();
    let (capture, factory) = capture_factory(false);
    let event_sink = Arc::new(ChannelEventSink::new(1024));
    let events = event_sink.subscribe();

    let player = Player::with_config(
        &cx,
        PlayerConfig::default()
            .audio_sink_factory(factory)
            .event_sink(Some(event_sink))
            .min_buffer_duration(min_buffer),
    )
    .expect("player construction failed");

    let src: Box<dyn AudioSource> = Box::new(BufferedSource::with_readahead(readahead, fake));
    if autoplay {
        player.load_and_play(src).expect("load");
    } else {
        player.load(src).expect("load");
    }

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
    // The starved window surfaces as Buffering (the sink keeps draining
    // its ring); not a frozen-but-Playing position.
    assert_eq!(harness.player.state(), PlayerState::Buffering);

    h.release_gate();
    assert!(
        wait_until(Duration::from_secs(3), || {
            harness.player.state() == PlayerState::Playing
        }),
        "refill must morph back to Playing"
    );
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
    // The failing source settles back to Playing (silent) — possibly via
    // a brief Buffering while the sticky error lands — and stays there.
    assert!(
        wait_until(Duration::from_secs(2), || {
            harness.player.state() == PlayerState::Playing
        }),
        "persistent errors must stay playing-silent (state: {:?})",
        harness.player.state()
    );

    // The failure is surfaced — exactly once (dedup latch): a failing
    // source errors on every pump; the event stream must carry one
    // `Error` for the whole episode, not a flood. (Drain-and-count —
    // a "wait for Ended" helper would consume the Error events.)
    let mut errors = 0;
    let mut saw_ended = false;
    let end = std::time::Instant::now() + Duration::from_millis(300);
    while std::time::Instant::now() < end {
        match harness.events.recv_timeout(Duration::from_millis(50)) {
            Ok(PlayerEvent::Error(_)) => errors += 1,
            Ok(PlayerEvent::Ended) => saw_ended = true,
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(!saw_ended, "persistent session errors must not emit Ended");
    assert_eq!(errors, 1, "expected exactly one Error event, got {errors}");
}

#[test]
fn buffered_position_tracks_the_window_in_media_time() {
    // `Player::buffered_position` is the bytes→media-time projection of
    // the mirrored window: `Some` and inside (0, duration] once the
    // readahead fills, and exactly the duration once everything has been
    // delivered (decode reached EOF, the window spans the tail).
    let data = wav(2.0);
    let (fake, _h) = fake(Arc::clone(&data)).chunk(4 * 1024).finish();
    let harness = harness_with(fake, 16 * 1024);
    let duration = harness.player.duration().expect("wav carries a duration");

    harness.player.play().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || harness
            .player
            .buffered_position()
            .is_some_and(|p| p > Duration::ZERO && p <= duration)),
        "buffered position must land inside (0, duration]: {:?}",
        harness.player.buffered_position()
    );

    assert!(
        wait_for_ended(&harness.events, Duration::from_secs(10)),
        "play-through must end"
    );
    // Everything delivered: the buffered frontier is the whole track.
    assert_eq!(
        harness.player.buffered_position(),
        Some(duration),
        "frontier must reach the duration at EOF (window: {:?})",
        harness.player.buffered_range()
    );
}

// ============================================================================
// B/O — min-buffer gating (startup prebuffer + refill threshold)
// ============================================================================

/// WAV byte rate in the fakes (44.1 kHz stereo 16-bit ≈ 176.4 KB/s): a
/// 2 s cushion needs ~352.8 KB, so a readahead of 400 KB can reach the
/// 2 s threshold while the legacy 16 KB cannot (~90 ms).
const TWO_SECS_OF_WAV: u64 = 352_800;
const _: () = assert!(TWO_SECS_OF_WAV > 64 * 1024);

#[test]
fn autoplay_prebuffers_before_playing_when_the_window_is_thin() {
    // Only the first ~0.57 s has been "delivered": the autoplay load
    // must park in `Buffering` instead of playing the thin window out
    // (and stuttering). Raising the ceiling past the threshold lets
    // the 400 KB readahead cross 2 s and play the rest through,
    // bit-exact, on the same session.
    let data = wav(3.0);
    let (fake, h) = fake(Arc::clone(&data)).deliver_until(100_000).finish();
    let harness = harness_with_config(fake, 400 * 1024, Duration::from_secs(2), true);

    // `load_and_play` returned with the startup prebuffer armed — the
    // morph happened synchronously in the load command.
    assert_eq!(harness.player.state(), PlayerState::Buffering);
    // It holds while the cushion is below the threshold (readiness is
    // already `Ready` here — the gate is the cushion, not the bytes).
    let mut stayed = true;
    let hold_until = std::time::Instant::now() + Duration::from_millis(400);
    while std::time::Instant::now() < hold_until {
        if harness.player.state() != PlayerState::Buffering {
            stayed = false;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(stayed, "must stay Buffering while the cushion is below 2 s");

    h.raise_deliver_until(data.len() as u64);
    assert!(
        wait_until(Duration::from_secs(3), || {
            harness.player.state() == PlayerState::Playing
        }),
        "refill must cross the threshold and morph back to Playing"
    );
    assert!(wait_for_ended(&harness.events, Duration::from_secs(10)));

    let captured = harness.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured, reference, "prebuffered audio must be bit-exact");
    assert_eq!(h.opens(), vec![0], "one session per play-through");
}

#[test]
fn refill_below_the_threshold_stays_parked() {
    // The gate's discriminator, pinned: once data IS flowing again
    // (ceiling raised to the whole file), a readahead window that can
    // only hold ~0.37 s of media still may not resume — the threshold
    // is 2 s. Under the pre-threshold behavior this resumes on the
    // first available byte.
    let data = wav(3.0);
    let (fake, h) = fake(Arc::clone(&data)).deliver_until(44_100).finish();
    let harness = harness_with_config(fake, 64 * 1024, Duration::from_secs(2), true);

    assert_eq!(harness.player.state(), PlayerState::Buffering);
    h.raise_deliver_until(data.len() as u64);
    let mut stayed = true;
    let hold_until = std::time::Instant::now() + Duration::from_millis(1200);
    while std::time::Instant::now() < hold_until {
        if harness.player.state() != PlayerState::Buffering {
            stayed = false;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        stayed,
        "a window that cannot reach the threshold must not resume (state: {:?})",
        harness.player.state()
    );
    assert!(
        harness
            .player
            .buffered_position()
            .is_some_and(|p| p < Duration::from_secs(2)),
        "the parked window stays below the threshold: {:?}",
        harness.player.buffered_position()
    );
}

#[test]
fn near_tail_refill_resumes_via_the_stream_end() {
    // A stall near the end of the track leaves less than the threshold
    // remaining (~0.57 s of a 4 s track): once the ceiling rises, the
    // window's end reaches the stream total and the refill must resume
    // regardless of the cushion — short tails never wedge in
    // `Buffering`. Playback starts un-gated here (not an autoplay load,
    // and the window already holds ~3.4 s ≥ 2 s), so this also covers
    // starving mid-play.
    let data = wav(4.0);
    let cap = (data.len() - 100_000) as u64;
    let (fake, h) = fake(Arc::clone(&data)).deliver_until(cap).finish();
    let harness = harness_with_config(fake, 400 * 1024, Duration::from_secs(2), false);

    harness.player.play().unwrap();
    assert!(
        wait_until(Duration::from_secs(6), || {
            harness.player.state() == PlayerState::Buffering
        }),
        "decode must starve at the ceiling (state: {:?})",
        harness.player.state()
    );

    h.raise_deliver_until(data.len() as u64);
    assert!(
        wait_until(Duration::from_secs(3), || {
            harness.player.state() == PlayerState::Playing
        }),
        "end-of-stream window must resume despite the thin cushion"
    );
    assert!(wait_for_ended(&harness.events, Duration::from_secs(10)));

    let captured = harness.capture.lock().unwrap().samples.clone();
    let reference = reference_decode(&data);
    assert_eq!(captured, reference, "tail audio must be bit-exact");
}
