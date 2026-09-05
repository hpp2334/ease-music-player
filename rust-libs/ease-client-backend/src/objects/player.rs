//! The audio-player object graph exposed over UniFFI.
//!
//! Three pieces:
//!
//! - [`PlayerContextHandle`] — wraps [`cantode::PlayerContext`]; owns the cpal
//!   `Host` and the shared decoder factory. One per app.
//! - [`PlayerHandle`] — shared [`cantode::Player`] (lock-free access; see
//!   its docs for why no `Mutex` may wrap it), one per app.
//!   Holds one worker thread.
//! - [`remote_music_source`] — builds a [`cantode::BufferedSource`] (the
//!   cantode-owned windowed source) over an [`AssetRemoteAudio`]
//!   long-lived session provider backed by [`services::get_asset_file`].
//!   All trait methods are non-blocking and run on cantode's session
//!   thread; the response body is forwarded on the shared tokio runtime,
//!   gated by cantode's demand (the byte source itself never crosses the
//!   FFI boundary — UniFFI can't express `Box<dyn AudioSource>`).
//!
//! The cover-art writeback hook lives in [`ct_player_load_music`]: if the
//! probed metadata carries embedded artwork and the DB's `Music.cover` is
//! `None`, a background tokio task writes the bytes via the existing
//! [`services::update_music_cover`] path.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use cantode::{AudioSource, BufferedSource, Pushed, RemoteAudioSource, StreamReply};
use ease_client_schema::{DataSourceKey, MusicId};
use ease_client_tokio::tokio_runtime;

use crate::{
    error::BError,
    objects::music::MetadataRecord,
    services::{
        get_asset_file, get_music, update_music_cover, update_music_duration, ArgUpdateMusicCover,
        ArgUpdateMusicDuration,
    },
    Backend, BackendContext,
};

// ============================================================================
// remote_music_source — cantode::BufferedSource over the backend's asset seam
// ============================================================================

/// One live streaming session: cantode's reply, the asset stream's chunk
/// receiver, and the demand gate the forward task parks on.
struct Session {
    reply: StreamReply,
    /// Outstanding demand in bytes — `request` raises it, the forward
    /// task consumes it as pushes are accepted.
    demand: std::sync::atomic::AtomicUsize,
    /// Set when cantode closed/superseded this session; wakes the parked
    /// forward task so it returns and drops the receiver — which aborts
    /// the underlying HTTP request (the producer task exits once its
    /// sends fail).
    dead: std::sync::atomic::AtomicBool,
    notify: tokio::sync::Notify,
}

impl Session {
    fn retire(&self) {
        self.dead.store(true, std::sync::atomic::Ordering::Release);
        self.notify.notify_one();
    }
}

/// A [`RemoteAudioSource`] over the backend's own asset seam: one
/// long-lived `get_asset_file` request per session, its body forwarded
/// exactly as cantode demands it.
///
/// This is the whole network-side integration — the windowing, demand
/// scheduling, seek cancellation, retries, and EOF validation live in
/// cantode. The bridge only maps the session lifecycle onto the storage
/// seam: `open` spawns the ranged request, `request` opens the demand
/// gate, `close` marks the session dead (tearing the transport down).
struct AssetRemoteAudio {
    cx: Arc<BackendContext>,
    key: DataSourceKey,
    cur: Mutex<Option<Arc<Session>>>,
}

impl RemoteAudioSource for AssetRemoteAudio {
    fn open(&self, offset: u64, reply: StreamReply) {
        // Retire any current session (normally cantode already closed it;
        // retiring again is an idempotent no-op).
        if let Some(old) = self.cur.lock().unwrap().take() {
            old.retire();
        }
        let session = Arc::new(Session {
            reply: reply.clone(),
            demand: std::sync::atomic::AtomicUsize::new(0),
            dead: std::sync::atomic::AtomicBool::new(false),
            notify: tokio::sync::Notify::new(),
        });
        *self.cur.lock().unwrap() = Some(Arc::clone(&session));

        // Fire-and-forget on the shared runtime; `open` itself must
        // return immediately (cantode's session thread is calling).
        let cx = Arc::clone(&self.cx);
        let key = self.key.clone();
        let forward_session = Arc::clone(&session);
        tokio_runtime().spawn(async move {
            match get_asset_file(&cx, key, offset).await {
                Ok(Some(file)) => {
                    if let Some(total) = file.total_size() {
                        reply.set_total_len(Some(total as u64));
                    }
                    let rx = file.into_rx();
                    tokio::spawn(forward_chunks(forward_session, rx));
                }
                Ok(None) => {
                    reply.finish_error("asset not found for source".into());
                }
                Err(e) => {
                    reply.finish_error(format!("asset open: {e:?}"));
                }
            }
        });
    }

    fn request(&self, want: usize) {
        if let Some(s) = self.cur.lock().unwrap().as_ref() {
            s.demand
                .fetch_add(want, std::sync::atomic::Ordering::AcqRel);
            s.notify.notify_one();
        }
    }

    fn close(&self) {
        if let Some(s) = self.cur.lock().unwrap().take() {
            s.retire();
        }
    }
}

/// The demand-gated copy loop: pull the response body and push exactly as
/// much as cantode demanded — no more, so the stream idles mid-body while
/// the window is full (TCP backpressure doing the work).
///
/// End-of-stream mapping: a channel close becomes `finish_eof` — and
/// because cantode knows the reported total length, an EOF that lands
/// short of it is retried as a failed session instead of surfacing as a
/// premature `Ended`. Over-delivery beyond the accepted push is kept as
/// the leftover tail and re-offered against future demand.
async fn forward_chunks(
    session: Arc<Session>,
    rx: async_channel::Receiver<ease_remote_storage::StorageBackendResult<Bytes>>,
) {
    // Bytes a partially-accepted push rejected (the "keep the tail"
    // contract) plus any chunk that raced ahead of demand.
    let mut leftover: Option<Bytes> = None;
    let mut want: usize = 0;
    loop {
        if session.dead.load(std::sync::atomic::Ordering::Acquire) {
            return; // drops rx → the producer task aborts the request
        }
        want += session
            .demand
            .swap(0, std::sync::atomic::Ordering::AcqRel);

        // Serve the outstanding demand: leftover first, then fresh chunks.
        while want > 0 {
            let chunk = match leftover.take() {
                Some(c) => c,
                None => match rx.recv().await {
                    Ok(Ok(c)) => c,
                    Ok(Err(e)) => {
                        session.reply.finish_error(format!("storage: {e:?}"));
                        return;
                    }
                    Err(_) => {
                        session.reply.finish_eof();
                        return;
                    }
                },
            };
            if chunk.is_empty() {
                continue;
            }
            let take = chunk.len().min(want);
            match session.reply.push(chunk[..take].to_vec()) {
                Pushed::Superseded => return,
                Pushed::Accepted(n) => {
                    want -= n;
                    if n < chunk.len() {
                        // Rejected tail: keep it for the next demand.
                        leftover = Some(chunk.slice(n..));
                    }
                }
            }
        }

        // No demand left: park until more demand arrives, the session is
        // retired, or (when the tail is empty) the stream itself ends.
        // Holding a leftover means we cannot pull another chunk without
        // over-consuming the body positionally, so only demand/death can
        // wake us — the close signal surfaces once the tail drains.
        if leftover.is_some() {
            session.notify.notified().await;
        } else {
            tokio::select! {
                _ = session.notify.notified() => {}
                res = rx.recv() => match res {
                    Ok(Ok(c)) => leftover = Some(c),
                    Ok(Err(e)) => {
                        session.reply.finish_error(format!("storage: {e:?}"));
                        return;
                    }
                    Err(_) => {
                        session.reply.finish_eof();
                        return;
                    }
                },
            }
        }
    }
}

/// Build the byte source for a music asset: a [`BufferedSource`] (the
/// cantode-owned windowed source) over an [`AssetRemoteAudio`] session
/// provider.
pub(crate) fn remote_music_source(cx: Arc<BackendContext>, key: DataSourceKey) -> BufferedSource {
    BufferedSource::new(Box::new(AssetRemoteAudio {
        cx,
        key,
        cur: Mutex::new(None),
    }))
}

// ============================================================================
// PlayerContextHandle — wraps cantode::PlayerContext
// ============================================================================

/// Owns the shared [`cantode::PlayerContext`] (cpal Host + decoder factory).
///
/// Construct once per app via [`ct_player_context_new`]. Players created
/// from this context share the cpal Host and the symphonia decoder factory.
pub struct PlayerContextHandle {
    inner: cantode::PlayerContext,
}

impl PlayerContextHandle {
    /// Number of live players created from this context.
    pub fn active_player_count(&self) -> u64 {
        self.inner.active_player_count() as u64
    }
}

impl PlayerContextHandle {
    /// Crate-internal constructor. The public FFI entry is the free fn
    /// `ct_player_context_new` so it matches the `ct_*` controller
    /// convention.
    pub(crate) fn new() -> Result<Self, BError> {
        let inner = cantode::PlayerContext::new().map_err(|e| BError::CustomError {
            message: format!("PlayerContext::new: {e:?}"),
        })?;
        Ok(Self { inner })
    }

    pub(crate) fn context(&self) -> &cantode::PlayerContext {
        &self.inner
    }
}

// ============================================================================
// PlayerHandle — shared cantode::Player
// ============================================================================

/// One audio playback pipeline. Holds a [`cantode::Player`] directly — no
/// Mutex: every `Player` method takes `&self` and is internally
/// thread-safe (commands serialize in the worker's channel; state and
/// position reads are lock-free). Wrapping it in a Mutex would starve
/// the 10 Hz `player.pollState` poll behind a blocking `loadMusic`
/// (the load holds the lock for its entire duration), making `Loading`
/// unobservable — exactly the bug this type must never reintroduce.
///
/// Create via [`ct_player_new`]; load a music with
/// [`ct_player_load_music`]; drive with `ct_player_play` /
/// `ct_player_pause` / `ct_player_stop` / `ct_player_seek` /
/// `ct_player_set_volume`.
pub struct PlayerHandle {
    player: Arc<cantode::Player>,
}

impl PlayerHandle {
    pub(crate) fn new(cx: &PlayerContextHandle) -> Result<Self, BError> {
        let player = cantode::Player::new(cx.context()).map_err(|e| BError::CustomError {
            message: format!("Player::new: {e:?}"),
        })?;
        Ok(Self {
            player: Arc::new(player),
        })
    }

    pub(crate) fn with_player<R>(&self, f: impl FnOnce(&cantode::Player) -> R) -> R {
        f(&self.player)
    }

    /// The shared [`Arc<Player>`] — handed to cantode's own FFI registry
    /// (feature `ffi`) so the Kotlin facade can address this player by the
    /// same bridge handle id. The registry keeps a `Weak`; this handle
    /// remains the owner.
    pub(crate) fn player_arc(&self) -> &Arc<cantode::Player> {
        &self.player
    }
}

// ============================================================================
// Player-side FFI controllers (ct_player_*)
// ============================================================================

use crate::error::BResult;

/// Construct the shared [`PlayerContextHandle`]. Call once at app start;
/// pass to `ct_player_new`.
///
/// This is a sync fn — `PlayerContext::new` opens the cpal default host
/// (AAudio on Android, CoreAudio on macOS, WASAPI on Windows, ALSA on
/// Linux) and constructs a `SymphoniaDecoderFactory`; both are cheap.
pub fn ct_player_context_new() -> BResult<Arc<PlayerContextHandle>> {
    Ok(Arc::new(PlayerContextHandle::new()?))
}

/// Construct a new [`PlayerHandle`] attached to `cx`. Each player owns
/// one dedicated decode/output worker thread.
pub fn ct_player_new(cx: Arc<PlayerContextHandle>) -> BResult<Arc<PlayerHandle>> {
    Ok(Arc::new(PlayerHandle::new(&cx)?))
}

/// Load (and replace any current source on) `player` with the bytes for
/// `music_id`. Returns the probed metadata. Does not start playback —
/// follow with `ct_player_play`.
///
/// Runs on the shared tokio pool via `spawn_blocking`: `Player::load`
/// opens the decoder + output device, which can take 10-500ms on slow
/// networks (the first chunk must arrive before symphonia can probe).
pub async fn ct_player_load_music(
    backend: Arc<Backend>,
    player: Arc<PlayerHandle>,
    music_id: MusicId,
    autoplay: bool,
) -> BResult<MetadataRecord> {
    // Bridge the sync `Player::load` onto the tokio pool so we don't
    // stall the UniFFI thread. The closure captures a clone of `backend`
    // (cheap `Arc` bump) and resolves the BackendContext inside. The
    // original `backend` is retained for the cover-writeback spawn below.
    let backend_for_closure = Arc::clone(&backend);
    let join_result = tokio_runtime()
        .handle()
        .spawn_blocking(move || -> Result<cantode::Metadata, BError> {
            let backend_cx = backend_for_closure.get_context().clone();
            let source: Box<dyn AudioSource> = Box::new(remote_music_source(
                Arc::new(backend_cx),
                DataSourceKey::Music { id: music_id },
            ));
            let player_inner = &player.player;
            // Autoplay: the load completes straight into `Playing` — the
            // caller must NOT follow with `player.play`, which would be
            // observable as a `Paused` park between the two. This blocks
            // the calling thread only — `with_player` is lock-free, so
            // the 10 Hz poll keeps observing `Loading` throughout.
            let result = if autoplay {
                player_inner.load_and_play(source)
            } else {
                player_inner.load(source)
            };
            result.map_err(|e| BError::CustomError {
                message: format!("Player::load: {e:?}"),
            })
        })
        .await;
    let metadata = join_result.map_err(|e| BError::CustomError {
        message: format!("join: {e}"),
    })??;

    // Metadata writeback: if the probe found embedded cover AND/OR a
    // duration that the DB doesn't yet have, fire-and-forget a tokio task
    // to fill them in. The UI reads duration from `music.meta.duration`
    // (the DB column), so without this writeback newly-imported tracks
    // show "--:--:--" until they're played for the first time. Cover
    // writeback also goes through here. Best-effort — failures are
    // logged, not surfaced.
    let has_cover = metadata.cover_art.is_some();
    let probed_duration = metadata.duration;
    if has_cover || probed_duration.is_some() {
        let backend_weak = backend.get_context().weak();
        let mid = music_id;
        let cover_bytes = metadata.cover_art.as_ref().map(|c| c.data.clone());
        tokio_runtime().handle().spawn(async move {
            let Some(cx) = backend_weak.upgrade() else {
                return;
            };
            let Ok(Some(m)) = get_music(&cx, mid).await else {
                return;
            };
            // Duration: only write if the DB column is currently null and
            // the probe produced a non-zero duration. Overwriting an
            // existing value would be surprising for users who manually
            // fixed it.
            if let Some(dur) = probed_duration {
                if !dur.is_zero() && m.meta.duration.is_none() {
                    let _ = update_music_duration(
                        &cx,
                        ArgUpdateMusicDuration {
                            id: mid,
                            duration: dur,
                        },
                    )
                    .await;
                }
            }
            // Cover: only write if the DB has no cover blob yet.
            if let Some(bytes) = cover_bytes {
                if m.cover.is_none() {
                    let _ = update_music_cover(
                        &cx,
                        ArgUpdateMusicCover {
                            id: mid,
                            cover: bytes,
                        },
                    )
                    .await;
                }
            }
        });
    }

    Ok(MetadataRecord::from_cantode(&metadata))
}

// Transport commands (play/pause/stop/seek/setVolume) and the state/
// position/duration observables moved to cantode's own FFI surface
// (`cantode::ffi`, feature `ffi`) — the Kotlin facade
// (`com.kutedev.cantode.CantodeNative`) calls them directly by the same
// bridge handle id (registered in `player.new` below). The load stays
// here: source construction and the metadata→DB writeback are backend
// (business) logic, per the cantode-owns-its-wire split.

/// Register the freshly created player with cantode's FFI registry under
/// `key`, so the Kotlin facade can address it. Idempotent.
pub fn ct_player_register_ffi(player: &Arc<PlayerHandle>, key: u64) {
    cantode::ffi::register_player(key, player.player_arc());
}


/// Probe the duration (in ms) of a music WITHOUT playing it and WITHOUT
/// opening any output device. Reuses the shared `PlayerContextHandle`'s
/// decoder factory so probe and play agree on codecs.
///
/// Used by the playlist-import path to pre-fill durations for newly-added
/// musics that don't have one in their container tags yet. Safe to call
/// while another music is playing — this does not touch the playback
/// pipeline.
///
/// Returns `None` if the music's container doesn't advertise a duration
/// (rare; most MP3/FLAC/M4A files do).
pub async fn ct_player_probe_duration_ms(
    cx: Arc<PlayerContextHandle>,
    backend: Arc<Backend>,
    music_id: MusicId,
) -> BResult<Option<u64>> {
    let backend_cx = backend.get_context().clone();
    let cx_inner = Arc::clone(&cx);
    let metadata = tokio_runtime()
        .handle()
        .spawn_blocking(move || -> Result<cantode::Metadata, BError> {
            let source: Box<dyn AudioSource> = Box::new(remote_music_source(
                Arc::new(backend_cx),
                DataSourceKey::Music { id: music_id },
            ));
            cantode::probe_metadata(&cx_inner.context(), source).map_err(|e| BError::CustomError {
                message: format!("probe_metadata: {e:?}"),
            })
        })
        .await
        .map_err(|e| BError::CustomError {
            message: format!("join: {e}"),
        })??;
    Ok(metadata.duration.map(|d| d.as_millis() as u64))
}
