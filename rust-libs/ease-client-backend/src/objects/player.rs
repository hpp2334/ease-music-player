//! The audio-player object graph exposed over UniFFI.
//!
//! Three pieces:
//!
//! - [`PlayerContextHandle`] — wraps [`cantode::PlayerContext`]; owns the cpal
//!   `Host` and the shared decoder factory. One per app.
//! - [`PlayerHandle`] — wraps [`cantode::Player`] behind a `Mutex` so it can
//!   be shared across UniFFI calls. Holds one worker thread.
//! - [`remote_music_source`] — builds a [`cantode::RemoteSource`] (the
//!   cantode-owned windowed source) over the backend's own
//!   [`services::get_asset_file`] seam. The fetch closure runs on the shared
//!   tokio runtime; the byte source itself never crosses the FFI boundary
//!   (UniFFI can't express `Box<dyn AudioSource>`).
//!
//! The cover-art writeback hook lives in [`ct_player_load_music`]: if the
//! probed metadata carries embedded artwork and the DB's `Music.cover` is
//! `None`, a background tokio task writes the bytes via the existing
//! [`services::update_music_cover`] path.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use cantode::{AudioSource, RemoteSource};
use ease_client_schema::{DataSourceKey, MusicId};
use ease_client_tokio::tokio_runtime;

use crate::{
    error::BError,
    objects::music::{MetadataRecord, PlayerStateRecord},
    services::{
        get_asset_file, get_music, update_music_cover, update_music_duration, ArgUpdateMusicCover,
        ArgUpdateMusicDuration,
    },
    Backend, BackendContext,
};

// ============================================================================
// remote_music_source — cantode::RemoteSource over the backend's asset seam
// ============================================================================

/// Build the byte source for a music asset: a [`RemoteSource`] whose fetch
/// closure opens the asset stream at the requested offset on the shared
/// tokio runtime and forwards chunks until `max_len`.
///
/// This is the whole network-side integration — the windowing, retries,
/// seek cancellation, and EOF validation live in cantode. The closure only
/// bridges: `get_asset_file` → `StreamFile` receiver → `ReplyHandle`.
pub(crate) fn remote_music_source(cx: Arc<BackendContext>, key: DataSourceKey) -> RemoteSource {
    RemoteSource::new(move |offset, max_len, reply| {
        let cx = Arc::clone(&cx);
        let key = key.clone();
        let reply = reply.clone();
        // Fire-and-forget on the shared runtime; the closure itself must
        // return immediately (the RemoteSource prefetch thread is calling).
        let _ = tokio_runtime().spawn(async move {
            match get_asset_file(&cx, key, offset).await {
                Ok(Some(file)) => {
                    if let Some(total) = file.total_size() {
                        reply.set_total_len(Some(total as u64));
                    }
                    forward_chunks(file.into_rx(), max_len, &reply).await;
                }
                Ok(None) => {
                    reply.finish_error("asset not found for source".into());
                }
                Err(e) => {
                    reply.finish_error(format!("asset open: {e:?}"));
                }
            }
        });
    })
}

/// Forward receiver chunks onto the reply until `max_len` bytes are
/// delivered, the stream ends, or it errors.
///
/// End-of-stream mapping: a channel close after a partial delivery becomes
/// `finish_eof` — and because the adapter knows the reported total length,
/// an EOF that lands short of it is retried as a failed range instead of
/// surfacing as a premature `Ended` (the phantom-Ended fix). Exactly
/// `max_len` delivered means the resource may continue, so no finish is
/// sent; the adapter requests the next range when it needs it.
async fn forward_chunks(
    mut rx: async_channel::Receiver<ease_remote_storage::StorageBackendResult<Bytes>>,
    mut remaining: usize,
    reply: &cantode::ReplyHandle,
) {
    while remaining > 0 {
        match rx.recv().await {
            Ok(Ok(chunk)) => {
                if chunk.is_empty() {
                    continue;
                }
                let take = chunk.len().min(remaining);
                reply.push_chunk(chunk[..take].to_vec());
                remaining -= take;
                if take < chunk.len() {
                    // Served beyond the request — drop the rest; the
                    // adapter will re-fetch from the right offset.
                    return;
                }
            }
            Ok(Err(e)) => {
                reply.finish_error(format!("storage: {e:?}"));
                return;
            }
            Err(_) => {
                reply.finish_eof();
                return;
            }
        }
    }
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
// PlayerHandle — wraps cantode::Player behind a Mutex
// ============================================================================

/// One audio playback pipeline. Wraps [`cantode::Player`] behind a `Mutex`
/// so multiple UniFFI calls can share it.
///
/// Create via [`ct_player_new`]; load a music with
/// [`ct_player_load_music`]; drive with `ct_player_play` /
/// `ct_player_pause` / `ct_player_stop` / `ct_player_seek` /
/// `ct_player_set_volume`.
pub struct PlayerHandle {
    inner: Mutex<cantode::Player>,
}

impl PlayerHandle {
    pub(crate) fn new(cx: &PlayerContextHandle) -> Result<Self, BError> {
        let player = cantode::Player::new(cx.context()).map_err(|e| BError::CustomError {
            message: format!("Player::new: {e:?}"),
        })?;
        Ok(Self {
            inner: Mutex::new(player),
        })
    }

    pub(crate) fn with_player<R>(&self, f: impl FnOnce(&cantode::Player) -> R) -> R {
        f(&self.inner.lock().unwrap())
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
            let player_inner = player.inner.lock().unwrap();
            player_inner.load(source).map_err(|e| BError::CustomError {
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

/// Begin or resume playback.
pub async fn ct_player_play(player: Arc<PlayerHandle>) -> BResult<()> {
    player.with_player(|p| {
        p.play().map_err(|e| BError::CustomError {
            message: format!("Player::play: {e:?}"),
        })
    })
}

/// Pause playback.
pub async fn ct_player_pause(player: Arc<PlayerHandle>) -> BResult<()> {
    player.with_player(|p| {
        p.pause().map_err(|e| BError::CustomError {
            message: format!("Player::pause: {e:?}"),
        })
    })
}

/// Stop playback and drop the loaded source (back to Idle).
pub async fn ct_player_stop(player: Arc<PlayerHandle>) -> BResult<()> {
    player.with_player(|p| {
        p.stop().map_err(|e| BError::CustomError {
            message: format!("Player::stop: {e:?}"),
        })
    })
}

/// Seek to `pos_ms` milliseconds from source start. Returns the actual
/// position seeked to (also in ms).
pub async fn ct_player_seek(player: Arc<PlayerHandle>, pos_ms: u64) -> BResult<u64> {
    let target = Duration::from_millis(pos_ms);
    let actual = player.with_player(|p| {
        p.seek(target).map_err(|e| BError::CustomError {
            message: format!("Player::seek: {e:?}"),
        })
    })?;
    Ok(actual.as_millis() as u64)
}

/// Set linear gain. `1.0` = unity, `0.0` = silent.
pub async fn ct_player_set_volume(player: Arc<PlayerHandle>, volume: f32) -> BResult<()> {
    player.with_player(|p| {
        p.set_volume(volume).map_err(|e| BError::CustomError {
            message: format!("Player::set_volume: {e:?}"),
        })
    })
}

/// Current externally-observable state.
pub fn ct_player_state(player: Arc<PlayerHandle>) -> PlayerStateRecord {
    player.with_player(|p| PlayerStateRecord::from_cantode(p.state()))
}

/// Current playback position in milliseconds.
pub fn ct_player_position_ms(player: Arc<PlayerHandle>) -> u64 {
    player.with_player(|p| p.position().as_millis() as u64)
}

/// Total duration of the loaded source in milliseconds, if known.
pub fn ct_player_duration_ms(player: Arc<PlayerHandle>) -> Option<u64> {
    player.with_player(|p| p.duration().map(|d| d.as_millis() as u64))
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
