//! The ready-made network [`AudioSource`]: long-lived streaming sessions
//! in, a windowed buffer out.
//!
//! [`BufferedSource`] is for embedders whose bytes arrive slower than RAM
//! — WebDAV, OneDrive, plain HTTP — expressed the way HTTP actually
//! works: **one long-lived request per session, streamed to the end**.
//! The embedder implements [`RemoteAudioSource`], a trait with an
//! explicit session lifecycle (`open` / `request` / `close`), and hands
//! it to [`BufferedSource::new`]. cantode owns everything else: the
//! readahead window, demand scheduling, seek cancellation, retries, EOF
//! validation, and a no-progress watchdog.
//!
//! ```text
//! cantode (session thread; never blocks)   biz (async, own runtime)
//! ─────────────────────────────────────    ─────────────────────────────
//! needs data at offset ── open(off,reply) ► issue one Range request ──┐
//!                          request(W) ───► read W from the same body   │ one
//!   (W = readahead − buffered_ahead −     push chunks via the reply   │ long-lived
//!    outstanding demand; rises as reads    body ends → finish_eof     │ request per
//!    drain; withheld while window full)                                 │ session,
//! read() parks on cantode's condvar ◄───── push / eof / error wake it │ streamed
//! seek in-window ── cursor move only; session lives                    │ to the end)
//! seek out-of-window ── close() ─► abort the transport ───────────────┘
//!                        └─ open(target, reply) ─► new request
//! stall / failed session ── close() + open(window_end)  (retry-bounded)
//! ```
//!
//! Ownership consequences worth stating:
//!
//! - **Every trait method is non-blocking.** `open` initiates, `request`
//!   grants demand, `close` aborts — none of them ever wait for I/O, so
//!   cantode never runs embedder code that could hang its threads. All
//!   waiting lives in [`Read`]'s condvar park (the player-worker side).
//!   Cancellation is ordinary task teardown on the embedder's side.
//! - **Demand telling.** `request(want)` is the byte size cantode needs
//!   for the current session; the embedder reads exactly that much from
//!   its established response and no more. While the window is full
//!   cantode simply doesn't call `request` — the connection idles
//!   mid-body, which is TCP backpressure doing the work.
//! - **EOF is only trusted when consistent.** A [`StreamReply::finish_eof`]
//!   that lands short of a reported [`StreamReply::set_total_len`] is
//!   treated as a failed session and retried — a dropped connection no
//!   longer masquerades as the end of the stream.
//! - **Seeks cancel in-flight work.** A seek outside the window supersedes
//!   the live session (late deliveries are generation-dropped) and the
//!   session thread closes it before opening the target range; a seek
//!   landing inside the window is a cursor move with no network at all.
//!
//! Total length is *discovered*, not declared: HTTP only learns
//! `Content-Length` when the response lands, so demanding it up front
//! would force a blocking round-trip before playback could start. Until
//! the first report, [`AudioSource::len`] returns `None` and
//! `SeekFrom::End` is rejected (mirroring the graceful degradation the
//! trait documents).

use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use super::{AudioSource, Readiness};

/// Default readahead: the buffered-ahead bytes the session tries to
/// maintain beyond the read cursor. Several seconds-to-a-minute of
/// compressed audio.
const DEFAULT_READAHEAD_BYTES: usize = 4 * 1024 * 1024;

/// Additional session attempts after a failure (error, lying EOF, stall)
/// before the source goes sticky-error. The next seek resets the budget.
const MAX_SESSION_RETRIES: u32 = 3;

/// Default no-progress watchdog: a live session with outstanding demand
/// that delivers nothing for this long is considered hung — it is closed
/// (which aborts the transport) and the range is retried. Progress
/// refreshes on every accepted push, so a slow-but-flowing stream never
/// trips it.
const DEFAULT_WATCHDOG: Duration = Duration::from_secs(30);

/// How long the session thread parks when idle (nothing to schedule, no
/// watchdog to watch). Pure watchdog; every state change notifies.
const PARK_IDLE: Duration = Duration::from_secs(60 * 60);

/// How long a deadline-less read parks before re-checking its condition.
/// Functionally "forever"; the re-loop keeps the code uniform.
const PARK_READ: Duration = Duration::from_secs(60 * 60);

/// A biz-implemented long-lived byte provider, driven by cantode.
///
/// One live session at a time. The impl maps the trait 1:1 onto async
/// I/O it already owns: `open` issues one ranged HTTP request (spawn; no
/// waiting), `request` reads more of the *same* response body, `close`
/// aborts it (drop the task/transport).
///
/// **All methods are non-blocking**: they initiate, signal, or abort —
/// they never wait for I/O, and they must return promptly. Session
/// progress (total length, bytes, EOF, error) is delivered asynchronously
/// through the [`StreamReply`] handed to `open`.
///
/// Threading contract:
///
/// - `open` / `request` / `close` are called only from cantode's session
///   thread, serialized, and **never while cantode holds its state
///   lock**. Impls may therefore deliver synchronously — e.g. `request`
///   pushing an already-buffered chunk straight through the reply —
///   without deadlocking.
/// - [`StreamReply`] methods may be called from any thread/task of the
///   impl's choosing; they are non-blocking and generation-guarded
///   no-ops once the session is superseded (seeked away / closed /
///   replaced).
/// - Impls use interior mutability (`&self`); the object is shared
///   between cantode's session thread and the impl's delivery tasks.
///
/// Cancellation is by teardown, not by interruption: when cantode
/// abandons a session it has not reported a terminal (`finish_eof` /
/// `finish_error`) for — an out-of-window seek, a stall, or shutdown —
/// it calls `close`, which must make the impl drop the transport (and
/// with it the HTTP request). A session that already reported a terminal
/// has ended itself; `close` is not required for it (but must remain a
/// tolerant no-op if called anyway). `close` is idempotent.
pub trait RemoteAudioSource: Send + Sync + 'static {
    /// Begin a session streaming from `offset`. Returns immediately after
    /// initiation — the response establishes asynchronously. Later
    /// reports flow through `reply`: `set_total_len` once known
    /// (Content-Length; never called = unknown length, e.g. a live
    /// stream), `push` per chunk, then exactly one of `finish_eof` /
    /// `finish_error` to end the session. `request` may arrive before the
    /// session is established — queue the demand.
    fn open(&self, offset: u64, reply: StreamReply);

    /// Demand: cantode wants up to `want` more bytes for the current
    /// session. Returns immediately; the next deliveries through the
    /// session's reply satisfy it. `want` is the byte size cantode needs
    /// for the current request — the impl reads exactly that much from
    /// its response body and no more. Not called before the session's
    /// `open` nor after `close`. Demands are **additive deltas** on one
    /// scalar (the impl's outstanding-read allowance): successive calls
    /// before satisfaction just raise the total; there is no per-request
    /// lifecycle, hence no request id — deliveries are positional
    /// (append at the window end) and staleness is session-scoped (the
    /// reply's generation guard).
    fn request(&self, want: usize);

    /// Abort the current session. Returns immediately; the session is
    /// dead from cantode's side (late deliveries are generation-dropped).
    /// The impl cancels its async work — dropping the task tears the
    /// transport down. Called when cantode abandons a live session
    /// (out-of-window seek, stall recovery, `Drop`); idempotent.
    fn close(&self);
}

/// The outcome of a [`StreamReply::push`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pushed {
    /// `n` bytes were accepted into the window. `n < bytes.len()` means
    /// the excess was **rejected, not truncated** — keep the tail and
    /// deliver it against future demand (this only happens to impls that
    /// push beyond the granted demand, e.g. after a backward in-window
    /// seek shrank the space).
    Accepted(usize),
    /// The session is gone (superseded by a seek, closed, or replaced).
    /// Abort; further deliveries are no-ops.
    Superseded,
}

/// The delivery channel for one [`RemoteAudioSource`] session.
///
/// Cloneable and `Send`; call from whichever task/thread runs the
/// session. Every method is non-blocking and a no-op once the session is
/// superseded (seeked away / closed / replaced).
#[derive(Clone)]
pub struct StreamReply {
    shared: Arc<Shared>,
    /// Generation (session epoch) this reply belongs to.
    session_gen: u64,
}

/// The buffered, seekable [`AudioSource`] cantode builds over a
/// [`RemoteAudioSource`]. Hand to [`Player::load`](crate::Player::load)
/// like any other source.
///
/// Construct with [`BufferedSource::new`] (default 4 MiB readahead) or
/// [`BufferedSource::with_readahead`]; [`BufferedSource::watchdog`] tunes
/// the no-progress timeout (rarely needed — mainly tests).
pub struct BufferedSource {
    shared: Arc<Shared>,
    /// The session thread; joined on drop (which also closes the live
    /// session).
    bg: Option<JoinHandle<()>>,
}

struct Shared {
    st: Mutex<Inner>,
    cv: Condvar,
}

struct Inner {
    /// Session epoch: bumped on every session change (abandon, reopen).
    /// Deliveries carrying an older generation are dropped.
    session_gen: u64,
    /// Contiguous window bytes starting at absolute `window_start`.
    window: Vec<u8>,
    window_start: u64,
    /// Read cursor (absolute). Invariant: `window_start <= pos <=
    /// window_start + window.len()`.
    pos: u64,
    /// Reported resource length, once discovered.
    total_len: Option<u64>,
    /// Trusted end-of-resource offset, once known (a consistent
    /// `finish_eof`, or implied by reaching `total_len`).
    eof: Option<u64>,
    /// A session is live: opened, no terminal reported, not superseded.
    session_open: bool,
    /// The live session was abandoned by the consumer (out-of-window
    /// seek) — the session thread must `close()` it before scheduling
    /// the next one.
    close_pending: bool,
    /// Demand granted to the live session (`request` deltas) minus bytes
    /// accepted since. Scheduling only grants the difference between the
    /// window's free space and this.
    outstanding: usize,
    /// Stamp of the last accepted delivery; arms the watchdog while
    /// demand is outstanding.
    last_progress: Instant,
    /// Failures since the last seek.
    retries: u32,
    /// Sticky failure after the retry budget is exhausted; cleared by the
    /// next seek.
    error: Option<String>,
    shutdown: bool,
    /// Play-path read deadline (see [`AudioSource::set_read_deadline`]):
    /// a parked read returns `WouldBlock` once this elapses. `None` parks
    /// until data arrives.
    read_deadline: Option<Instant>,
    /// Readahead in bytes; also the per-session space cap and (×2) the
    /// window-retention cap.
    readahead: usize,
    /// No-progress timeout for a live session with outstanding demand.
    watchdog: Duration,
}

impl BufferedSource {
    /// Build a source with the default readahead
    /// (`DEFAULT_READAHEAD_BYTES`, 4 MiB).
    ///
    /// The first session at offset 0 is opened immediately, so a
    /// following [`Player::load`](crate::Player::load) probe doesn't wait
    /// for the session to *start* (it still waits for the first bytes —
    /// it must read them).
    pub fn new(inner: Box<dyn RemoteAudioSource>) -> Self {
        Self::with_readahead(DEFAULT_READAHEAD_BYTES, inner)
    }

    /// Build a source with an explicit readahead in bytes — also the
    /// per-session space cap and half the window-retention cap. Smaller
    /// values trade network round-trips for memory (and make
    /// backpressure observable with smaller resources).
    pub fn with_readahead(readahead_bytes: usize, inner: Box<dyn RemoteAudioSource>) -> Self {
        let readahead = readahead_bytes.max(1);
        let shared = Arc::new(Shared {
            st: Mutex::new(Inner {
                session_gen: 0,
                window: Vec::new(),
                window_start: 0,
                pos: 0,
                total_len: None,
                eof: None,
                session_open: false,
                close_pending: false,
                outstanding: 0,
                last_progress: Instant::now(),
                retries: 0,
                error: None,
                shutdown: false,
                read_deadline: None,
                readahead,
                watchdog: DEFAULT_WATCHDOG,
            }),
            cv: Condvar::new(),
        });

        let thread_shared = Arc::clone(&shared);
        let bg = std::thread::Builder::new()
            .name("cantode-remote-session".into())
            .spawn(move || session_loop(thread_shared, inner))
            .expect("spawn cantode-remote-session");

        Self {
            shared,
            bg: Some(bg),
        }
    }

    /// Override the no-progress watchdog (default 30 s). Builder-style;
    /// call before handing the source to the player. Production embedders
    /// rarely need this — it exists so tests can observe stall recovery
    /// quickly.
    pub fn watchdog(self, timeout: Duration) -> Self {
        self.shared.st.lock().unwrap().watchdog = timeout.max(Duration::from_millis(1));
        self
    }

    /// The currently-loaded window: `(offset, bytes)` — `bytes` of
    /// contiguous data are buffered starting at absolute `offset`.
    ///
    /// Intended for embedder UI ("buffered amount" bars). The offset
    /// only moves forward (consumed-prefix eviction); a seek resets it to
    /// the seek target.
    pub fn loaded_window(&self) -> (u64, usize) {
        let st = self.shared.st.lock().unwrap();
        (st.window_start, st.window.len())
    }
}

impl Drop for BufferedSource {
    fn drop(&mut self) {
        // Stop the session thread and wait for the in-flight trait call
        // to return (all methods are non-blocking, so this is prompt).
        // The thread's exit path closes the live session, aborting the
        // transport.
        {
            let mut st = self.shared.st.lock().unwrap();
            st.shutdown = true;
        }
        self.shared.cv.notify_all();
        if let Some(bg) = self.bg.take() {
            let _ = bg.join();
        }
    }
}

impl Read for BufferedSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut st = self.shared.st.lock().unwrap();
        loop {
            if let Some(err) = st.error.clone() {
                return Err(io::Error::other(err));
            }
            let window_end = st.window_start + st.window.len() as u64;
            if st.pos < window_end {
                let off = (st.pos - st.window_start) as usize;
                let n = buf.len().min(st.window.len() - off);
                buf[..n].copy_from_slice(&st.window[off..off + n]);
                st.pos += n as u64;
                // Evict the consumed prefix once the retained window
                // exceeds the cap, so memory stays bounded on long (or
                // infinite) streams. Never evicts past the cursor.
                let cap = 2 * st.readahead;
                if st.window.len() > cap {
                    let drop = (st.pos - st.window_start) as usize;
                    st.window.drain(..drop);
                    st.window_start = st.pos;
                }
                // Progress: the window's free space grew.
                self.shared.cv.notify_all();
                return Ok(n);
            }
            // Cursor at the window end with no data available: genuine
            // EOF boundaries, else park for the session.
            if st.eof.is_some_and(|e| st.pos >= e) || st.total_len.is_some_and(|t| st.pos >= t) {
                return Ok(0);
            }
            // Play-path deadline: bound the park so a starved read
            // surfaces as `WouldBlock` instead of blocking the pump
            // forever (probe/load reads leave it unset and park).
            if let Some(deadline) = st.read_deadline {
                let now = Instant::now();
                if now >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "read deadline elapsed before data arrived",
                    ));
                }
                let (guard, _result) = self.shared.cv.wait_timeout(st, deadline - now).unwrap();
                st = guard;
            } else {
                st = self.shared.cv.wait_timeout(st, PARK_READ).unwrap().0;
            }
        }
    }
}

impl Seek for BufferedSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let mut st = self.shared.st.lock().unwrap();
        let total = st.total_len;
        let target = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(d) => d.checked_add(st.pos as i64).map(|n| n.max(0) as u64),
            SeekFrom::End(d) => total.map(|t| (t as i64 + d).max(0) as u64),
        };
        let Some(mut target) = target else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek from end with unknown length",
            ));
        };
        if let Some(total) = total {
            target = target.min(total);
        }

        let window_end = st.window_start + st.window.len() as u64;
        let in_window = target >= st.window_start && target < window_end;
        if !in_window {
            // Reset the window to the target — one contiguous session
            // from here, no stitching with stale bytes.
            st.window.clear();
            st.window_start = target;
        }
        st.pos = target;
        // Supersede in-flight work: older generations are dropped, and
        // the (now dead) live session is closed before the next opens.
        st.session_gen += 1;
        if st.session_open {
            st.session_open = false;
            st.close_pending = true;
        }
        st.outstanding = 0;
        // A fresh seek epoch also resets the retry budget and clears any
        // sticky error — give the network a fresh chance.
        st.retries = 0;
        st.error = None;
        self.shared.cv.notify_all();
        Ok(target)
    }
}

impl AudioSource for BufferedSource {
    fn len(&self) -> Option<u64> {
        self.shared.st.lock().unwrap().total_len
    }

    fn readiness(&self) -> Readiness {
        let st = self.shared.st.lock().unwrap();
        let window_end = st.window_start + st.window.len() as u64;
        let starved = st.pos >= window_end
            && st.error.is_none()
            && st.eof.is_none_or(|e| st.pos < e)
            && st.total_len.is_none_or(|t| st.pos < t)
            && !st.shutdown;
        if starved {
            Readiness::NeedsData
        } else {
            Readiness::Ready
        }
    }

    fn set_read_deadline(&mut self, deadline: Option<Duration>) {
        let mut st = self.shared.st.lock().unwrap();
        st.read_deadline = deadline.map(|d| Instant::now() + d);
        self.shared.cv.notify_all();
    }
}

// ============================================================================
// Session thread
// ============================================================================

/// What the session thread should do next.
enum Action {
    /// Shutdown: close the live session (if any) and exit.
    Exit,
    /// An abandoned session needs its `close()`.
    Close,
    /// Open a fresh session at the offset.
    Open(u64),
    /// Grant `want` more bytes of demand to the live session.
    Request(usize),
    /// Stall recovery: close the live session, then reopen at the offset.
    Reopen(u64),
}

/// The session loop: schedule sessions and demand, watch for stalls,
/// retry failed ranges. Never blocks inside embedder code.
fn session_loop(shared: Arc<Shared>, inner: Box<dyn RemoteAudioSource>) {
    loop {
        match decide(&shared) {
            Action::Exit => {
                call_close(inner.as_ref());
                return;
            }
            Action::Close => call_close(inner.as_ref()),
            Action::Open(offset) => open_session(&shared, inner.as_ref(), offset),
            Action::Request(want) => {
                if let Err(panic) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.request(want)))
                {
                    let mut st = shared.st.lock().unwrap();
                    fail_session(&mut st, format!("request panicked: {panic:?}"));
                    drop(st);
                    shared.cv.notify_all();
                }
            }
            Action::Reopen(offset) => {
                call_close(inner.as_ref());
                open_session(&shared, inner.as_ref(), offset);
            }
        }
    }
}

/// Compute the next [`Action`], parking until there is one. All state
/// decisions happen under the lock; the resulting trait call happens
/// outside it (impls may re-enter synchronously through the reply).
fn decide(shared: &Arc<Shared>) -> Action {
    let mut st = shared.st.lock().unwrap();
    loop {
        if st.shutdown {
            return Action::Exit;
        }
        if st.close_pending {
            st.close_pending = false;
            return Action::Close;
        }
        // Watchdog: a live session with outstanding demand that has
        // delivered nothing since `last_progress` is hung. Abandon it
        // (late deliveries die with the generation bump) and retry the
        // range, budget permitting.
        if st.session_open && st.outstanding > 0 && st.last_progress.elapsed() >= st.watchdog {
            let offset = st.window_start + st.window.len() as u64;
            st.session_gen += 1;
            st.outstanding = 0;
            st.last_progress = Instant::now();
            st.retries += 1;
            if st.retries > MAX_SESSION_RETRIES {
                st.error = Some("session stalled: no delivery progress before the watchdog".into());
                st.session_open = false;
                st.close_pending = true;
                // Wake parked readers: the sticky error is now visible to
                // them (`notify_all` while holding the lock is fine —
                // waiters re-acquire on wake).
                shared.cv.notify_all();
            } else {
                return Action::Reopen(offset);
            }
        }
        let window_end = st.window_start + st.window.len() as u64;
        let at_eof = st.eof.is_some_and(|e| window_end >= e)
            || st.total_len.is_some_and(|t| window_end >= t);
        let ahead = window_end.saturating_sub(st.pos);
        let space = st.readahead.saturating_sub(ahead as usize);
        if st.error.is_none() && !at_eof && space > 0 {
            if !st.session_open {
                // No live session and the window wants bytes: open one at
                // the window end. (Deliberately not when `space == 0` —
                // a failed session behind a full window reopens only
                // once reads drain it.)
                st.session_gen += 1;
                st.session_open = true;
                st.outstanding = 0;
                st.last_progress = Instant::now();
                return Action::Open(window_end);
            }
            if space > st.outstanding {
                let want = space - st.outstanding;
                st.outstanding += want;
                return Action::Request(want);
            }
        }
        // Nothing to do: park until a state change — or the watchdog, if
        // armed (demand outstanding on a live session).
        let timeout = if st.session_open && st.outstanding > 0 {
            st.watchdog.saturating_sub(st.last_progress.elapsed())
        } else {
            PARK_IDLE
        };
        let (guard, _result) = shared.cv.wait_timeout(st, timeout).unwrap();
        st = guard;
    }
}

/// Invoke `inner.open(offset, reply)` for the current generation,
/// treating a panic as a failed session.
fn open_session(shared: &Arc<Shared>, inner: &dyn RemoteAudioSource, offset: u64) {
    let session_gen = shared.st.lock().unwrap().session_gen;
    let reply = StreamReply {
        shared: Arc::clone(shared),
        session_gen,
    };
    if let Err(panic) =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.open(offset, reply)))
    {
        let mut st = shared.st.lock().unwrap();
        fail_session(&mut st, format!("open panicked: {panic:?}"));
        drop(st);
        shared.cv.notify_all();
    }
}

/// A trait call panicked, or the session otherwise failed out-of-band:
/// kill the session (generation bump drops its late deliveries) and count
/// the failure against the budget. Caller notifies.
fn fail_session(st: &mut Inner, reason: String) {
    st.session_gen += 1;
    st.session_open = false;
    st.close_pending = true;
    st.outstanding = 0;
    st.retries += 1;
    if st.retries > MAX_SESSION_RETRIES {
        st.error = Some(reason);
    }
}

/// `close()` an abandoned session; a panicking close is logged and
/// swallowed (nothing downstream depends on it).
fn call_close(inner: &dyn RemoteAudioSource) {
    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.close())) {
        tracing::warn!("RemoteAudioSource::close panicked: {panic:?}");
    }
}

// ============================================================================
// StreamReply — deliveries from the impl's session
// ============================================================================

impl StreamReply {
    /// Report what this session learned about the resource size
    /// (`Content-Length`, PROPFIND, ...). Callable any time, from any
    /// session of the current generation; later reports overwrite
    /// earlier ones. Never called at all = unknown length (live
    /// streams).
    ///
    /// This is what [`AudioSource::len`] exposes, and what validates
    /// EOFs: an EOF that lands short of a reported length is retried,
    /// not trusted — including one that was trusted before the total
    /// arrived.
    pub fn set_total_len(&self, total: Option<u64>) {
        let mut st = self.shared.st.lock().unwrap();
        if st.session_gen != self.session_gen {
            return;
        }
        st.total_len = total;
        // An earlier trusted EOF that now lands short of the reported
        // length was a failed session, not the end: undo and retry.
        let window_end = st.window_start + st.window.len() as u64;
        if let (Some(eof), Some(t)) = (st.eof, total)
            && eof < t
            && eof == window_end
        {
            st.eof = None;
            st.session_open = false;
            st.retries += 1;
            if st.retries > MAX_SESSION_RETRIES {
                st.error = Some("stream ended short of reported total length".into());
            }
        }
        self.shared.cv.notify_all();
    }

    /// Deliver fetched bytes, appending at the window end.
    ///
    /// Returns how many were accepted — never a silent truncation:
    /// [`Pushed::Accepted`] with `n < bytes.len()` means the excess was
    /// *rejected* (keep the tail); [`Pushed::Superseded`] means the
    /// session is gone. An impl that reads only its granted demand never
    /// sees a partial acceptance outside of seek races.
    pub fn push(&self, bytes: Vec<u8>) -> Pushed {
        let mut st = self.shared.st.lock().unwrap();
        if st.session_gen != self.session_gen || !st.session_open {
            return Pushed::Superseded;
        }
        let window_end = st.window_start + st.window.len() as u64;
        let ahead = window_end.saturating_sub(st.pos);
        let mut cap = st.readahead.saturating_sub(ahead as usize);
        if let Some(total) = st.total_len {
            cap = cap.min(total.saturating_sub(window_end) as usize);
        }
        let n = bytes.len().min(cap);
        if n > 0 {
            st.window.extend_from_slice(&bytes[..n]);
            st.outstanding = st.outstanding.saturating_sub(n);
            st.last_progress = Instant::now();
            self.shared.cv.notify_all();
        }
        Pushed::Accepted(n)
    }

    /// Signal that the resource ends at the last delivered byte.
    ///
    /// Honored only when consistent: an EOF short of a reported total
    /// length is treated as a failed session and retried (bounded by the
    /// retry budget). For resources of unknown length the EOF is trusted
    /// as-is.
    pub fn finish_eof(&self) {
        let mut st = self.shared.st.lock().unwrap();
        if st.session_gen != self.session_gen || !st.session_open {
            return;
        }
        st.session_open = false;
        st.outstanding = 0;
        let window_end = st.window_start + st.window.len() as u64;
        if st.total_len.is_some_and(|t| window_end < t) {
            // Died short of a known length: failed session, not EOF.
            st.retries += 1;
            if st.retries > MAX_SESSION_RETRIES {
                st.error = Some("stream ended short of reported total length".into());
            }
        } else {
            st.eof = Some(window_end);
        }
        self.shared.cv.notify_all();
    }

    /// Signal that this session failed. The adapter retries the range
    /// (bounded); exhausted retries make reads fail until the next seek
    /// resets the budget.
    pub fn finish_error(&self, err: String) {
        let mut st = self.shared.st.lock().unwrap();
        if st.session_gen != self.session_gen || !st.session_open {
            return;
        }
        st.session_open = false;
        st.outstanding = 0;
        st.retries += 1;
        if st.retries > MAX_SESSION_RETRIES {
            st.error = Some(err);
        }
        self.shared.cv.notify_all();
    }
}
