//! The ready-made network [`AudioSource`]: range fetches in, a windowed
//! buffer out.
//!
//! [`RemoteSource`] is for embedders whose bytes arrive slower than RAM
//! — WebDAV, OneDrive, plain HTTP — and who would otherwise have to
//! bridge an async client onto the sync `Read + Seek` seam themselves
//! (a ~200-line exercise in lookahead buffers, `block_on` bridges, and
//! seek re-opens). The split of responsibilities:
//!
//! - **The embedder** supplies only a *fetch closure* — "produce bytes
//!   starting at this offset, at most this many" — and runs it on
//!   whatever runtime it already has. It reports what it learns (total
//!   length) and what it produced (chunks, end, error) through the
//!   [`ReplyHandle`].
//! - **The adapter** owns everything else: the readahead window, fetch
//!   scheduling and retries, generation-based cancellation on seek,
//!   EOF validation against the reported length, and a no-progress
//!   deadline that turns a hung fetch into a retryable failure.
//!
//! Ownership consequences worth stating:
//!
//! - **The player's worker thread never fetches.** [`Read`] parks on a
//!   condvar while the window is empty and a fetch can still produce
//!   data; a dedicated prefetch thread does the fetching and deadline
//!   watching. A slow network degrades exactly like the stall case
//!   characterized in `tests/network_source.rs` (position freezes,
//!   commands queue behind the parked read), and a *hung* fetch is
//!   bounded by the fetch deadline plus the retry budget instead of
//!   parking the worker forever.
//! - **EOF is only trusted when it is consistent.** A
//!   [`ReplyHandle::finish_eof`] that lands short of a reported
//!   [`ReplyHandle::set_total_len`] is treated as a failed range and
//!   retried — a dropped HTTP chunk no longer masquerades as the end
//!   of the stream.
//! - **Seeks cancel in-flight work.** Each seek bumps a generation;
//!   deliveries carrying a stale generation (or a stale fetch attempt)
//!   are dropped, so rapid scrubbing fetches only the final range. A
//!   seek landing inside the window is a cursor move with no network
//!   at all.
//!
//! Total length is *discovered*, not declared: HTTP only learns
//! `Content-Length` when the first response lands, so demanding it up
//! front would force a blocking metadata round-trip before playback
//! could start. Until the first report, [`AudioSource::len`] returns
//! `None` and `SeekFrom::End` is rejected (mirroring the graceful
//! degradation the trait documents).

use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use super::AudioSource;

/// Default readahead: the buffered-ahead bytes the prefetch thread
/// tries to maintain beyond the read cursor, and the `max_len` it asks
/// each fetch for. Several seconds-to-a-minute of compressed audio.
const DEFAULT_READAHEAD_BYTES: usize = 4 * 1024 * 1024;

/// Additional fetch attempts after a failure before the source goes
/// sticky-error (until the next seek resets the budget).
const MAX_FETCH_RETRIES: u32 = 3;

/// A fetch that makes no progress (no chunk delivered) for this long is
/// considered hung and failed. Progress resets the clock on every
/// chunk, so a slow-but-flowing stream never trips it. This bounds the
/// "network hung without answering" case; it is deliberately generous.
const FETCH_DEADLINE: Duration = Duration::from_secs(30);

/// How long the prefetch thread parks when idle (nothing to fetch, no
/// deadline to watch). Pure watchdog; every state change notifies.
const PARK_IDLE: Duration = Duration::from_secs(60 * 60);

/// The fetch closure type: `(offset, max_len, reply)`.
type FetchFn = dyn Fn(u64, usize, ReplyHandle) + Send + Sync;

/// A buffered, seekable [`AudioSource`] over an embedder-provided
/// range-fetch closure.
///
/// See the [module docs](self) for the responsibility split and the
/// guarantees. Construct with [`RemoteSource::new`] (default readahead)
/// or [`RemoteSource::with_readahead`], then hand it to
/// [`Player::load`](crate::Player::load) like any other source.
pub struct RemoteSource {
    shared: Arc<Shared>,
    /// Readahead in bytes; also the per-fetch `max_len` and (×2) the
    /// window-retention cap.
    readahead: usize,
    /// The prefetch thread; joined on drop.
    bg: Option<JoinHandle<()>>,
}

/// The delivery channel from a fetch closure back to the adapter.
///
/// Cloneable and `Send`; call from whichever thread/runtime runs the
/// fetch. Every method is a no-op once the fetch's generation or
/// attempt has been superseded (seeked away, retried, or shutdown), so
/// late deliveries from cancelled work are safe.
#[derive(Clone)]
pub struct ReplyHandle {
    shared: Arc<Shared>,
    /// Generation (seek epoch) this reply belongs to.
    seek_gen: u64,
    /// Fetch-attempt id this reply belongs to.
    attempt: u64,
}

struct Shared {
    st: Mutex<Inner>,
    cv: Condvar,
}

struct Inner {
    /// Seek epoch: bumped on every seek; deliveries with an older
    /// generation are dropped.
    seek_gen: u64,
    /// Monotonic fetch-attempt id; identifies one in-flight (or just
    /// completed) fetch invocation.
    next_attempt: u64,
    /// Contiguous window bytes starting at absolute `window_start`.
    window: Vec<u8>,
    window_start: u64,
    /// Read cursor (absolute). Invariant: `window_start <= pos`.
    pos: u64,
    /// Reported resource length, once discovered.
    total_len: Option<u64>,
    /// Trusted end-of-resource offset, once known (via a consistent
    /// `finish_eof`, or implied by reaching `total_len`).
    eof: Option<u64>,
    /// The live fetch, if one has been initiated and not yet completed.
    outstanding: Option<Outstanding>,
    /// The most recent attempt that delivered its full `max_len` — its
    /// `finish_eof` is still honored (the resource may end exactly
    /// there), until superseded.
    completed_attempt: Option<u64>,
    /// Failures since the last seek.
    retries: u32,
    /// Sticky failure after the retry budget is exhausted; cleared by
    /// the next seek.
    error: Option<String>,
    shutdown: bool,
}

struct Outstanding {
    attempt: u64,
    /// Bytes this fetch may still deliver.
    remaining: usize,
    last_progress: Instant,
}

impl RemoteSource {
    /// Build a source with the default readahead
    /// (`DEFAULT_READAHEAD_BYTES`, 4 MiB).
    ///
    /// The first fetch at offset 0 is initiated immediately, so a
    /// following [`Player::load`](crate::Player::load) probe doesn't
    /// wait for the fetch to *start* (it still waits for the first
    /// bytes — it must read them).
    pub fn new(fetch: impl Fn(u64, usize, ReplyHandle) + Send + Sync + 'static) -> Self {
        Self::with_readahead(DEFAULT_READAHEAD_BYTES, fetch)
    }

    /// Build a source with an explicit readahead in bytes — also the
    /// per-fetch `max_len` and half the window-retention cap. Smaller
    /// values trade network round-trips for memory (and make
    /// backpressure observable with smaller resources).
    pub fn with_readahead(
        readahead_bytes: usize,
        fetch: impl Fn(u64, usize, ReplyHandle) + Send + Sync + 'static,
    ) -> Self {
        let readahead = readahead_bytes.max(1);
        let fetch: Arc<FetchFn> = Arc::new(fetch);
        let shared = Arc::new(Shared {
            st: Mutex::new(Inner {
                seek_gen: 0,
                next_attempt: 0,
                window: Vec::new(),
                window_start: 0,
                pos: 0,
                total_len: None,
                eof: None,
                outstanding: None,
                completed_attempt: None,
                retries: 0,
                error: None,
                shutdown: false,
            }),
            cv: Condvar::new(),
        });

        let thread_shared = Arc::clone(&shared);
        let bg = std::thread::Builder::new()
            .name("cantode-remote-prefetch".into())
            .spawn(move || prefetch_loop(thread_shared, fetch, readahead))
            .expect("spawn cantode-remote-prefetch");

        Self {
            shared,
            readahead,
            bg: Some(bg),
        }
    }

    /// The currently-loaded window: `(offset, bytes)` — `bytes` of
    /// contiguous data are buffered starting at absolute `offset`.
    ///
    /// Intended for embedder UI ("buffered amount" bars). The offset
    /// only moves forward (consumed-prefix eviction); a seek resets it
    /// to the seek target.
    pub fn loaded_window(&self) -> (u64, usize) {
        let st = self.shared.st.lock().unwrap();
        (st.window_start, st.window.len())
    }
}

impl Drop for RemoteSource {
    fn drop(&mut self) {
        // Stop the prefetch thread and wait for the in-flight fetch
        // closure call (embedder closures are expected to return
        // quickly — they spawn, they don't block).
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

impl Read for RemoteSource {
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
                // exceeds the cap, so memory stays bounded on long
                // (or infinite) streams. Never evicts past the cursor.
                let cap = 2 * self.readahead;
                if st.window.len() > cap {
                    let drop = (st.pos - st.window_start) as usize;
                    st.window.drain(..drop);
                    st.window_start = st.pos;
                }
                // Progress: the readahead gap widened.
                self.shared.cv.notify_all();
                return Ok(n);
            }
            // Cursor at/behind the window end and no data available:
            // genuine EOF boundaries, else park for the prefetch.
            if st.eof.is_some_and(|e| st.pos >= e) || st.total_len.is_some_and(|t| st.pos >= t) {
                return Ok(0);
            }
            st = self.shared.cv.wait(st).unwrap();
        }
    }
}

impl Seek for RemoteSource {
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
            // Reset the window to the target — one contiguous fetch
            // from here, no stitching with stale bytes.
            st.window.clear();
            st.window_start = target;
        }
        st.pos = target;
        // Cancel in-flight work and give the network a fresh chance:
        // older generations/attempts are now stale.
        st.seek_gen += 1;
        st.outstanding = None;
        st.completed_attempt = None;
        st.retries = 0;
        st.error = None;
        self.shared.cv.notify_all();
        Ok(target)
    }
}

impl AudioSource for RemoteSource {
    fn len(&self) -> Option<u64> {
        self.shared.st.lock().unwrap().total_len
    }
}

// ============================================================================
// Prefetch thread
// ============================================================================

/// The prefetch loop: initiate fetches while the readahead gap wants
/// filling, watch the no-progress deadline, retry failed ranges.
fn prefetch_loop(shared: Arc<Shared>, fetch: Arc<FetchFn>, readahead: usize) {
    loop {
        // Phase 1 — under the lock: enforce deadlines, then either
        // decide the next fetch or park until something changes.
        let (start, max_len, seek_gen, attempt) = {
            let mut st = shared.st.lock().unwrap();
            loop {
                if st.shutdown {
                    return;
                }
                if let Some(o) = &st.outstanding
                    && o.last_progress.elapsed() >= FETCH_DEADLINE
                {
                    let reason = "fetch deadline exceeded without progress".to_string();
                    range_failed(&mut st, reason);
                }
                let window_end = st.window_start + st.window.len() as u64;
                let at_eof = st.eof.is_some_and(|e| window_end >= e)
                    || st.total_len.is_some_and(|t| window_end >= t);
                let gap = window_end - st.pos;
                if st.outstanding.is_none()
                    && st.error.is_none()
                    && !at_eof
                    && gap < readahead as u64
                {
                    let max_len = match st.total_len {
                        Some(t) => (readahead as u64).min(t - window_end) as usize,
                        None => readahead,
                    };
                    let attempt = st.next_attempt;
                    st.next_attempt += 1;
                    st.completed_attempt = None;
                    st.outstanding = Some(Outstanding {
                        attempt,
                        remaining: max_len,
                        last_progress: Instant::now(),
                    });
                    break (window_end, max_len, st.seek_gen, attempt);
                }
                let timeout = match &st.outstanding {
                    Some(o) => FETCH_DEADLINE.saturating_sub(o.last_progress.elapsed()),
                    None => PARK_IDLE,
                };
                let (guard, _result) = shared.cv.wait_timeout(st, timeout).unwrap();
                st = guard;
                // A timed-out wait re-enters the loop, where the
                // deadline check at the top acts on it.
            }
        };

        // Phase 2 — outside the lock: the closure may re-enter
        // synchronously through the reply handle.
        let reply = ReplyHandle {
            shared: Arc::clone(&shared),
            seek_gen,
            attempt,
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fetch(start, max_len, reply)
        }));
        if let Err(panic) = outcome {
            let reason = format!("fetch closure panicked: {panic:?}");
            let mut st = shared.st.lock().unwrap();
            range_failed(&mut st, reason);
            shared.cv.notify_all();
        }
    }
}

/// A fetch failed (error, hang, or lying EOF). Either keep the retry
/// budget going or go sticky-error. Caller holds the lock.
fn range_failed(st: &mut Inner, reason: String) {
    st.outstanding = None;
    st.completed_attempt = None;
    st.retries += 1;
    if st.retries > MAX_FETCH_RETRIES {
        st.error = Some(reason);
    }
}

// ============================================================================
// ReplyHandle — deliveries from the embedder's fetch
// ============================================================================

impl ReplyHandle {
    /// Report what this fetch learned about the resource size
    /// (`Content-Length`, PROPFIND, ...). Callable any time, from any
    /// fetch of the current generation; later reports overwrite
    /// earlier ones. Never called at all = unknown length (live
    /// streams).
    ///
    /// This is what [`AudioSource::len`] exposes, and what validates
    /// EOFs: an EOF that lands short of a reported length is retried,
    /// not trusted.
    pub fn set_total_len(&self, total: Option<u64>) {
        let mut st = self.shared.st.lock().unwrap();
        if st.seek_gen != self.seek_gen {
            return;
        }
        st.total_len = total;
        // An earlier trusted EOF that now lands short of the reported
        // length was a failed range, not the end: undo and retry.
        let window_end = st.window_start + st.window.len() as u64;
        if let (Some(eof), Some(t)) = (st.eof, total)
            && eof < t
            && eof == window_end
        {
            st.eof = None;
            let reason = "stream ended short of reported total length".to_string();
            range_failed(&mut st, reason);
        }
        self.shared.cv.notify_all();
    }

    /// Deliver fetched bytes. Chunks append in order; delivery beyond
    /// the requested `max_len` is clamped.
    pub fn push_chunk(&self, mut bytes: Vec<u8>) {
        let mut st = self.shared.st.lock().unwrap();
        let is_live = st
            .outstanding
            .as_ref()
            .is_some_and(|o| o.attempt == self.attempt && o.remaining > 0);
        if !is_live {
            return; // superseded, completed, or unsolicited
        }
        {
            let o = st.outstanding.as_mut().expect("checked above");
            if bytes.len() > o.remaining {
                bytes.truncate(o.remaining);
            }
            o.remaining -= bytes.len();
            o.last_progress = Instant::now();
        }
        let completed = st.outstanding.as_ref().is_some_and(|o| o.remaining == 0);
        st.window.extend_from_slice(&bytes);
        if completed {
            st.completed_attempt = Some(self.attempt);
            st.outstanding = None;
        }
        self.shared.cv.notify_all();
    }

    /// Signal that the resource ends at the last delivered byte.
    ///
    /// Honored only when consistent: an EOF short of a reported total
    /// length is treated as a failed range and retried (bounded by the
    /// retry budget). For resources of unknown length the EOF is
    /// trusted as-is.
    pub fn finish_eof(&self) {
        let mut st = self.shared.st.lock().unwrap();
        if st.seek_gen != self.seek_gen {
            return;
        }
        let live = st
            .outstanding
            .as_ref()
            .is_some_and(|o| o.attempt == self.attempt);
        let just_completed = st.completed_attempt == Some(self.attempt);
        if !live && !just_completed {
            return;
        }
        st.outstanding = None;
        st.completed_attempt = None;
        let window_end = st.window_start + st.window.len() as u64;
        if let Some(t) = st.total_len
            && window_end < t
        {
            // Died short of a known length: failed range, not EOF.
            let reason = "stream ended short of reported total length".to_string();
            range_failed(&mut st, reason);
        } else {
            st.eof = Some(window_end);
        }
        self.shared.cv.notify_all();
    }

    /// Signal that this fetch failed. The adapter retries the range
    /// (bounded); exhausted retries make reads fail until the next
    /// seek resets the budget.
    pub fn finish_error(&self, err: String) {
        let mut st = self.shared.st.lock().unwrap();
        if st.seek_gen != self.seek_gen
            || st
                .outstanding
                .as_ref()
                .is_none_or(|o| o.attempt != self.attempt)
        {
            return;
        }
        range_failed(&mut st, err);
        self.shared.cv.notify_all();
    }
}
