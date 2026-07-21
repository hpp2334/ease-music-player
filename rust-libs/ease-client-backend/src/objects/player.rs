//! The audio-player object graph exposed over UniFFI.
//!
//! Three pieces:
//!
//! - [`PlayerContextHandle`] — wraps [`cantode::PlayerContext`]; owns the cpal
//!   `Host` and the shared decoder factory. One per app.
//! - [`PlayerHandle`] — wraps [`cantode::Player`] behind a `Mutex` so it can
//!   be shared across UniFFI calls. Holds one worker thread.
//! - [`MusicAudioSource`] — a [`cantode::AudioSource`] backed by the
//!   backend's own [`services::get_asset_file`] seam. This is the byte
//!   source the decoder reads from; it never crosses the FFI boundary
//!   itself (UniFFI can't express `Box<dyn AudioSource>`).
//!
//! The cover-art writeback hook lives in [`ct_player_load_music`]: if the
//! probed metadata carries embedded artwork and the DB's `Music.cover` is
//! `None`, a background tokio task writes the bytes via the existing
//! [`services::update_music_cover`] path.

use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use bytes::{Buf, Bytes};
use cantode::AudioSource;
use ease_client_schema::{DataSourceKey, MusicId};
use ease_client_tokio::tokio_runtime;
use ease_remote_storage::{StorageBackendResult, StreamFile};

use crate::{
    error::BError,
    objects::music::{MetadataRecord, PlayerStateRecord},
    services::{get_asset_file, get_music, update_music_cover, ArgUpdateMusicCover},
    Backend, BackendContext,
};

// ============================================================================
// MusicAudioSource — cantode::AudioSource impl over the backend's asset seam
// ============================================================================

/// A [`cantode::AudioSource`] that pulls bytes from the backend's cloud
/// storage via [`get_asset_file`].
///
/// `Read` is fed by an `async_channel::Receiver<Bytes>` (the same primitive
/// `ct_get_asset_stream` exposes); `Seek` drops the receiver and re-issues
/// `get_asset_file` at the new offset — the only way to seek a forward-only
/// HTTP response.
///
/// Block-on-tokio calls are safe here: this struct is only ever touched by
/// the cantode worker thread (a dedicated `std::thread`), never by a tokio
/// runtime thread, so we cannot deadlock the runtime.
pub(crate) struct MusicAudioSource {
    cx: Arc<BackendContext>,
    key: DataSourceKey,
    /// Active chunk receiver (None after EOF). Re-established by `open_stream_at`.
    rx: Mutex<Option<async_channel::Receiver<StorageBackendResult<Bytes>>>>,
    /// One-chunk lookahead buffer. `read` drains this first, then asks the
    /// receiver for more. Keeps the read path simple (never half-chunks).
    tail: Mutex<Bytes>,
    /// Position within the source, advanced by `read` and reset by `seek`.
    position: std::sync::atomic::AtomicU64,
    /// Total content length (bytes). Set on the first `open_stream_at` call.
    total_len: OnceLock<Option<u64>>,
}

impl MusicAudioSource {
    /// Open `key` starting at byte 0. Eagerly opens so the first `read`
    /// doesn't pay the open cost and `len()` is immediately known.
    pub(crate) fn new(cx: Arc<BackendContext>, key: DataSourceKey) -> io::Result<Self> {
        let this = Self {
            cx,
            key,
            rx: Mutex::new(None),
            tail: Mutex::new(Bytes::new()),
            position: std::sync::atomic::AtomicU64::new(0),
            total_len: OnceLock::new(),
        };
        this.open_stream_at(0)?;
        Ok(this)
    }

    /// (Re)open the underlying byte stream at `offset`. Drops any prior
    /// receiver + tail. Synchronously blocks on the tokio runtime — fine
    /// because we're on the cantode worker thread.
    fn open_stream_at(&self, offset: u64) -> io::Result<()> {
        let cx = self.cx.clone();
        let key = self.key.clone();
        let file: Option<StreamFile> = tokio_runtime()
            .handle()
            .block_on(async move { get_asset_file(&cx, key, offset).await })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("asset open: {e:?}")))?;

        let Some(file) = file else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "asset not found for source",
            ));
        };

        // Cache total length on first open; subsequent opens (seeks)
        // leave the cached value in place.
        let _ = self.total_len.set(file.total_size().map(|n| n as u64));

        let rx = file.into_rx();
        *self.rx.lock().unwrap() = Some(rx);
        *self.tail.lock().unwrap() = Bytes::new();
        self.position
            .store(offset, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Pull the next chunk from the receiver into `tail`. Returns `false`
    /// on EOF (channel closed) without error.
    fn refill_tail(&self) -> io::Result<bool> {
        let rx_opt = self.rx.lock().unwrap().take();
        let Some(rx) = rx_opt else {
            // No receiver means EOF was hit on a previous call.
            return Ok(false);
        };
        // `async_channel::Receiver::recv` takes `&self`, so we can borrow
        // `rx` from the closure (no `move`) and put it back afterwards.
        let recv_result = tokio_runtime()
            .handle()
            .block_on(async { rx.recv().await });
        match recv_result {
            Ok(chunk_result) => {
                let chunk: Bytes = chunk_result.map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, format!("storage: {e:?}"))
                })?;
                *self.tail.lock().unwrap() = chunk;
                // Put the receiver back so the next refill can continue.
                *self.rx.lock().unwrap() = Some(rx);
                Ok(true)
            }
            // Channel closed = EOF on the underlying stream.
            Err(_) => Ok(false),
        }
    }
}

impl Read for MusicAudioSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < buf.len() {
            // Drain tail first.
            let to_take = {
                let mut tail = self.tail.lock().unwrap();
                if tail.is_empty() {
                    0
                } else {
                    let to_take = (buf.len() - written).min(tail.len());
                    buf[written..written + to_take]
                        .copy_from_slice(&tail[..to_take]);
                    tail.advance(to_take);
                    to_take
                }
            };
            if to_take > 0 {
                written += to_take;
                self.position
                    .fetch_add(to_take as u64, std::sync::atomic::Ordering::Relaxed);
                continue;
            }

            // Tail empty: refill. If EOF, stop.
            if !self.refill_tail()? {
                break;
            }
        }
        Ok(written)
    }
}

impl Seek for MusicAudioSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let total_opt: Option<u64> = self.total_len.get().copied().flatten();
        let target = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(n) => {
                let cur = self
                    .position
                    .load(std::sync::atomic::Ordering::Relaxed);
                n.checked_add(cur as i64).map(|n| n.max(0) as u64)
            }
            SeekFrom::End(n) => total_opt.map(|t| {
                if n >= 0 {
                    t
                } else {
                    let target = t as i64 + n;
                    if target < 0 {
                        0
                    } else {
                        target as u64
                    }
                }
            }),
        };
        let Some(target) = target else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek from end with unknown length",
            ));
        };

        // Drop current stream and re-open at the new offset. Even for
        // target == current position we re-open: the tail may have stale
        // bytes from a different offset and reasoning about it precisely
        // isn't worth the complexity.
        self.open_stream_at(target)?;
        Ok(target)
    }
}

impl AudioSource for MusicAudioSource {
    fn len(&self) -> Option<u64> {
        self.total_len.get().copied().flatten()
    }
}

// ============================================================================
// PlayerContextHandle — wraps cantode::PlayerContext
// ============================================================================

/// Owns the shared [`cantode::PlayerContext`] (cpal Host + decoder factory).
///
/// Construct once per app via [`ct_player_context_new`]. Players created
/// from this context share the cpal Host and the symphonia decoder factory.
#[derive(uniffi::Object)]
pub struct PlayerContextHandle {
    inner: cantode::PlayerContext,
}

#[uniffi::export]
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
        let inner = cantode::PlayerContext::new()
            .map_err(|e| BError::CustomError { message: format!("PlayerContext::new: {e:?}") })?;
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
#[derive(uniffi::Object)]
pub struct PlayerHandle {
    inner: Mutex<cantode::Player>,
}

impl PlayerHandle {
    pub(crate) fn new(cx: &PlayerContextHandle) -> Result<Self, BError> {
        let player = cantode::Player::new(cx.context())
            .map_err(|e| BError::CustomError { message: format!("Player::new: {e:?}") })?;
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
#[uniffi::export]
pub fn ct_player_context_new() -> BResult<Arc<PlayerContextHandle>> {
    Ok(Arc::new(PlayerContextHandle::new()?))
}

/// Construct a new [`PlayerHandle`] attached to `cx`. Each player owns
/// one dedicated decode/output worker thread.
#[uniffi::export]
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
#[uniffi::export]
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
            let source: Box<dyn AudioSource> = match MusicAudioSource::new(
                Arc::new(backend_cx),
                DataSourceKey::Music { id: music_id },
            ) {
                Ok(s) => Box::new(s),
                Err(e) => {
                    return Err(BError::CustomError {
                        message: format!("MusicAudioSource::new: {e}"),
                    });
                }
            };
            let player_inner = player.inner.lock().unwrap();
            player_inner.load(source).map_err(|e| BError::CustomError {
                message: format!("Player::load: {e:?}"),
            })
        })
        .await;
    let metadata = join_result
        .map_err(|e| BError::CustomError { message: format!("join: {e}") })??;

    // Cover-art writeback: if the probe found embedded cover AND the
    // DB's Music.cover is None, fire-and-forget a tokio task to write
    // it. Best-effort — failures are logged, not surfaced.
    if metadata.cover_art.is_some() {
        let backend_weak = backend.get_context().weak();
        let mid = music_id;
        let cover = metadata.cover_art.clone().unwrap();
        tokio_runtime().handle().spawn(async move {
            if let Some(cx) = backend_weak.upgrade() {
                if let Ok(Some(m)) = get_music(&cx, mid).await {
                    if m.cover.is_none() {
                        let _ = update_music_cover(
                            &cx,
                            ArgUpdateMusicCover {
                                id: mid,
                                cover: cover.data.clone(),
                            },
                        )
                        .await;
                    }
                }
            }
        });
    }

    Ok(MetadataRecord::from_cantode(&metadata))
}

/// Begin or resume playback.
#[uniffi::export]
pub async fn ct_player_play(player: Arc<PlayerHandle>) -> BResult<()> {
    player.with_player(|p| {
        p.play()
            .map_err(|e| BError::CustomError { message: format!("Player::play: {e:?}") })
    })
}

/// Pause playback.
#[uniffi::export]
pub async fn ct_player_pause(player: Arc<PlayerHandle>) -> BResult<()> {
    player.with_player(|p| {
        p.pause()
            .map_err(|e| BError::CustomError { message: format!("Player::pause: {e:?}") })
    })
}

/// Stop playback and drop the loaded source (back to Idle).
#[uniffi::export]
pub async fn ct_player_stop(player: Arc<PlayerHandle>) -> BResult<()> {
    player.with_player(|p| {
        p.stop()
            .map_err(|e| BError::CustomError { message: format!("Player::stop: {e:?}") })
    })
}

/// Seek to `pos_ms` milliseconds from source start. Returns the actual
/// position seeked to (also in ms).
#[uniffi::export]
pub async fn ct_player_seek(player: Arc<PlayerHandle>, pos_ms: u64) -> BResult<u64> {
    let target = Duration::from_millis(pos_ms);
    let actual = player.with_player(|p| {
        p.seek(target)
            .map_err(|e| BError::CustomError { message: format!("Player::seek: {e:?}") })
    })?;
    Ok(actual.as_millis() as u64)
}

/// Set linear gain. `1.0` = unity, `0.0` = silent.
#[uniffi::export]
pub async fn ct_player_set_volume(player: Arc<PlayerHandle>, volume: f32) -> BResult<()> {
    player.with_player(|p| {
        p.set_volume(volume)
            .map_err(|e| BError::CustomError { message: format!("Player::set_volume: {e:?}") })
    })
}

/// Current externally-observable state.
#[uniffi::export]
pub fn ct_player_state(player: Arc<PlayerHandle>) -> PlayerStateRecord {
    player.with_player(|p| PlayerStateRecord::from_cantode(p.state()))
}

/// Current playback position in milliseconds.
#[uniffi::export]
pub fn ct_player_position_ms(player: Arc<PlayerHandle>) -> u64 {
    player.with_player(|p| p.position().as_millis() as u64)
}

/// Total duration of the loaded source in milliseconds, if known.
#[uniffi::export]
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
#[uniffi::export]
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
            let source: Box<dyn AudioSource> = match MusicAudioSource::new(
                Arc::new(backend_cx),
                DataSourceKey::Music { id: music_id },
            ) {
                Ok(s) => Box::new(s),
                Err(e) => {
                    return Err(BError::CustomError {
                        message: format!("MusicAudioSource::new: {e}"),
                    });
                }
            };
            cantode::probe_metadata(&cx_inner.context(), source).map_err(|e| BError::CustomError {
                message: format!("probe_metadata: {e:?}"),
            })
        })
        .await
        .map_err(|e| BError::CustomError { message: format!("join: {e}") })??;
    Ok(metadata.duration.map(|d| d.as_millis() as u64))
}
