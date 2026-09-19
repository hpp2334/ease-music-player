//! m4a-over-network regression tests: symphonia's isomp4 prologue must
//! not thrash HTTP range sessions.
//!
//! The isomp4 reader opens seekable sources by scanning ALL top-level
//! atoms — skipping past `mdat` with a real seek (any skip ≥ 2× the
//! 64 KiB `MediaSourceStream` ring seeks), then seeking back to byte 0
//! for a second pass that walks to the first `mdat` again. Over
//! [`BufferedSource`] every out-of-window seek used to mean: kill the
//! live HTTP session, *discard* the buffered window, re-fetch the range
//! from scratch. A network m4a therefore cost 3–4 range GETs per load
//! (header+moov, tail probe, header again, resume) while mp3/flac cost
//! exactly one — the "m4a buffers forever on WebDAV" bug.
//!
//! The fix lives in `BufferedSource::seek` (see `tests/buffered_source.rs`
//! for the unit pins): in-window seeks no longer supersede the live
//! session, and an out-of-window seek parks its window in a retention
//! slot that a later seek back into restores with no network. These
//! integration tests pin the end-to-end contract — decode an m4a
//! through the real demuxer over a counting source — for both committed
//! fixture layouts (`tests/fixtures/`, generated with ffmpeg; `mdat` is
//! ~247 KiB so the prologue's skip actually seeks):
//!
//! - `noise-10s-m4a-faststart.m4a` — moov before mdat ("Web-optimized").
//! - `noise-10s-m4a-moov-at-end.m4a` — moov after mdat (ffmpeg's
//!   default; the tail region is genuinely needed, so two fetch regions
//!   are inherent — but the header region still must not be re-fetched).
//!
//! Contract under load: at most 3 sessions per full load+decode, and
//! byte 0 fetched exactly once. (Before the fix: 4 sessions, byte 0
//! twice.) Decoded audio must be bit-identical to a memory-backed
//! decode of the same bytes.

use std::sync::{Arc, Mutex};

use cantode::{
    AudioSource, BufferedSource, Decoder, DecoderFactory, MemoryAudioSource, Pushed,
    RemoteAudioSource, StreamReply, SymphoniaDecoderFactory,
};

const FASTSTART: &[u8] = include_bytes!("fixtures/noise-10s-m4a-faststart.m4a");
const MOOV_AT_END: &[u8] = include_bytes!("fixtures/noise-10s-m4a-moov-at-end.m4a");

/// Readahead for the network source: 64 KiB — well under the fixtures'
/// ~247 KiB `mdat`, so the prologue's mdat skip always lands out of
/// window (the churn-triggering case), and small enough that playback
/// itself must resume on a continuation session.
const READAHEAD: usize = 64 * 1024;

// ============================================================================
// CountingRemote — a RAM source that logs every session open
// ============================================================================

/// One logged `open` == one HTTP range GET in the real embedding.
#[derive(Clone, Default)]
struct OpenLog {
    offsets: Arc<Mutex<Vec<u64>>>,
}

impl OpenLog {
    fn push(&self, offset: u64) {
        self.offsets.lock().unwrap().push(offset);
    }
    fn all(&self) -> Vec<u64> {
        self.offsets.lock().unwrap().clone()
    }
}

struct CountingRemoteState {
    /// Next byte to serve (advances by accepted bytes only).
    cursor: u64,
    /// The live session's reply (set at `open`, cleared at `close`).
    reply: Option<StreamReply>,
}

/// A [`RemoteAudioSource`] over in-RAM data that serves synchronously
/// (delivery from inside `request` is legal — cantode never holds its
/// state lock across trait calls) and logs each `open` offset.
struct CountingRemote {
    data: Arc<Vec<u8>>,
    log: OpenLog,
    st: Arc<Mutex<CountingRemoteState>>,
}

impl CountingRemote {
    fn new(data: Arc<Vec<u8>>, log: OpenLog) -> Box<Self> {
        Box::new(Self {
            data,
            log,
            st: Arc::new(Mutex::new(CountingRemoteState {
                cursor: 0,
                reply: None,
            })),
        })
    }

    /// Serve up to `want` bytes from the cursor in 8 KiB pushes,
    /// advancing by accepted bytes only (the rejected-tail contract).
    fn serve(&self, reply: &StreamReply, st: &mut CountingRemoteState, want: usize) {
        let end = (st.cursor + want as u64).min(self.data.len() as u64);
        let mut at = st.cursor;
        while at < end {
            let take = ((end - at) as usize).min(8 * 1024);
            match reply.push(self.data[at as usize..at as usize + take].to_vec()) {
                Pushed::Superseded => return,
                Pushed::Accepted(n) => {
                    at += n as u64;
                    if n < take {
                        // Rejected tail: keep it for future demand.
                        break;
                    }
                }
            }
        }
        st.cursor = at;
        if at >= self.data.len() as u64 {
            reply.finish_eof();
        }
    }
}

impl RemoteAudioSource for CountingRemote {
    fn open(&self, offset: u64, reply: StreamReply) {
        self.log.push(offset);
        let mut st = self.st.lock().unwrap();
        st.cursor = offset;
        reply.set_total_len(Some(self.data.len() as u64));
        st.reply = Some(reply);
    }

    fn request(&self, want: usize) {
        let mut st = self.st.lock().unwrap();
        let Some(reply) = st.reply.clone() else {
            return;
        };
        self.serve(&reply, &mut st, want);
    }

    fn close(&self) {
        self.st.lock().unwrap().reply = None;
    }
}

// ============================================================================
// Harness
// ============================================================================

/// One decoded frame, reduced to comparable data.
struct FrameSummary {
    timestamp: std::time::Duration,
    frames: usize,
    samples: Vec<f32>,
}

fn decode(dec: &mut dyn Decoder) -> Vec<FrameSummary> {
    let mut out = Vec::new();
    while let Some(frame) = dec
        .next_frame()
        .unwrap_or_else(|e| panic!("decode failed: {e:?}"))
    {
        out.push(FrameSummary {
            timestamp: frame.timestamp,
            frames: frame.frames,
            samples: frame.data,
        });
    }
    out
}

/// Decode the fixture from memory — the bit-exactness oracle.
fn reference_frames(data: &[u8]) -> Vec<FrameSummary> {
    let source: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(data.to_vec()));
    let mut dec = SymphoniaDecoderFactory::new()
        .open(source)
        .unwrap_or_else(|e| panic!("memory decoder open failed: {e:?}"));
    decode(dec.as_mut())
}

/// Decode the fixture through the real isomp4 prologue over a
/// `BufferedSource` backed by the counting remote; returns the frames
/// and every session-open offset, in order.
fn decode_over_network(data: &[u8]) -> (Vec<FrameSummary>, Vec<u64>) {
    let log = OpenLog::default();
    let remote = CountingRemote::new(Arc::new(data.to_vec()), log.clone());
    let source: Box<dyn AudioSource> = Box::new(BufferedSource::with_readahead(READAHEAD, remote));
    let mut dec = SymphoniaDecoderFactory::new()
        .open(source)
        .unwrap_or_else(|e| panic!("network decoder open failed: {e:?}"));
    let frames = decode(dec.as_mut());
    (frames, log.all())
}

fn assert_contract(name: &'static str, data: &'static [u8]) {
    let (frames, opens) = decode_over_network(data);
    let reference = reference_frames(data);

    assert!(!frames.is_empty(), "{name}: nothing decoded");
    assert_eq!(frames.len(), reference.len(), "{name}: frame count");
    for (i, (got, want)) in frames.iter().zip(&reference).enumerate() {
        assert_eq!(got.timestamp, want.timestamp, "{name}: frame {i} timestamp");
        assert_eq!(got.frames, want.frames, "{name}: frame {i} frame count");
        assert_eq!(got.samples, want.samples, "{name}: frame {i} samples");
    }

    // The churn contract. Session 1 reads the header (+moov for
    // faststart); session 2 is the prologue's tail probe (inherent to
    // "seekable source must scan trailing atoms" — fixing it needs
    // demuxer-side knowledge); session 3 resumes playback past the
    // restored window. A fourth session, or a second fetch of byte 0,
    // is the regression.
    assert_eq!(
        opens.first(),
        Some(&0),
        "{name}: first session at 0: {opens:?}"
    );
    assert!(
        opens.len() <= 3,
        "{name}: expected ≤ 3 sessions (header, tail probe, resume), got {opens:?}"
    );
    assert_eq!(
        log_count(&opens, 0),
        1,
        "{name}: byte 0 must be fetched exactly once: {opens:?}"
    );
}

fn log_count(opens: &[u64], offset: u64) -> usize {
    opens.iter().filter(|o| **o == offset).count()
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn faststart_m4a_loads_without_session_thrash() {
    assert_contract("faststart", FASTSTART);
}

#[test]
fn moov_at_end_m4a_loads_without_header_refetch() {
    assert_contract("moov-at-end", MOOV_AT_END);
}
