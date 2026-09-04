use std::time::Duration;

use ease_client_schema::{DataSourceKey, MusicId, MusicModel, PlaylistId, StorageEntryLoc};
use serde::{Deserialize, Serialize};

use crate::{
    ctx::BackendContext,
    error::BResult,
    objects::{LyricLoadState, Music, MusicAbstract, MusicLyric, MusicMeta},
    StorageEntry,
};

use super::{lyrics::parse_lrc, storage::load_storage_entry_data};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdatePlaylist {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToAddMusicEntry {
    pub entry: StorageEntry,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgCreatePlaylist {
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub entries: Vec<ToAddMusicEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgAddMusicsToPlaylist {
    pub id: PlaylistId,
    pub entries: Vec<ToAddMusicEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgRemoveMusicFromPlaylist {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicLyric {
    pub id: MusicId,
    pub lyric_loc: Option<StorageEntryLoc>,
}

async fn load_lyric(
    cx: &BackendContext,
    loc: Option<StorageEntryLoc>,
    is_fallback: bool,
) -> Option<MusicLyric> {
    let loc = match loc {
        Some(loc) => loc,
        None => {
            return None;
        }
    };
    let data = load_storage_entry_data(cx, &loc).await;
    if let Err(e) = &data {
        tracing::error!("fail to load entry {:?}: {}", loc, e);
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: if is_fallback {
                LyricLoadState::Missing
            } else {
                LyricLoadState::Failed
            },
        });
    }
    let data = data.unwrap();
    if data.is_none() {
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: if is_fallback {
                LyricLoadState::Missing
            } else {
                LyricLoadState::Failed
            },
        });
    }
    let data = data.unwrap();
    let data = String::from_utf8_lossy(&data).to_string();
    let lyric = parse_lrc(data);
    if lyric.is_err() {
        let e = lyric.unwrap_err();
        tracing::error!("fail to parse lyric: {}", e);
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: LyricLoadState::Failed,
        });
    }
    let lyric = lyric.unwrap();

    Some(MusicLyric {
        loc,
        data: lyric,
        loaded_state: LyricLoadState::Loaded,
    })
}

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
        order: model.order,
    }
}

pub(crate) fn build_music_abstract(_cx: &BackendContext, model: MusicModel) -> MusicAbstract {
    let cover = if model.cover.is_some() {
        Some(DataSourceKey::Cover { id: model.id })
    } else {
        Default::default()
    };

    MusicAbstract {
        cover,
        meta: build_music_meta(model),
    }
}

pub async fn get_music_storage_entry_loc(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<StorageEntryLoc>> {
    let m = cx.database_server().load_music(id).await?;
    if m.is_none() {
        return Ok(None);
    }
    let m = m.unwrap();
    let m = m.loc;
    Ok(Some(m))
}

pub async fn get_music_cover_bytes(cx: &BackendContext, id: MusicId) -> BResult<Vec<u8>> {
    let m = cx.database_server().load_music(id).await?.unwrap();
    if let Some(id) = m.cover {
        cx.database_server().blob().read(id)
    } else {
        Ok(Default::default())
    }
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicDuration {
    pub id: MusicId,
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub duration: Duration,
}
pub(crate) async fn update_music_duration(
    cx: &BackendContext,
    arg: ArgUpdateMusicDuration,
) -> BResult<()> {
    cx.database_server()
        .update_music_total_duration(arg.id, arg.duration)
        .await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicCover {
    pub id: MusicId,
    pub cover: Vec<u8>,
}
pub(crate) async fn update_music_cover(
    cx: &BackendContext,
    arg: ArgUpdateMusicCover,
) -> BResult<()> {
    cx.database_server()
        .update_music_cover(arg.id, arg.cover.clone())
        .await?;
    Ok(())
}

/// Resolve a music's lyric location: the explicit `model.lyric` if set,
/// else — when `lyric_default` is enabled — the sibling `.lrc` next to
/// the audio file. Returns `(loc, is_fallback)`.
fn resolve_lyric_loc(model: &MusicModel) -> (Option<StorageEntryLoc>, bool) {
    if let Some(loc) = model.lyric.clone() {
        return (Some(loc), false);
    }
    if !model.lyric_default {
        return (None, false);
    }
    let audio = &model.loc;
    let mut path = audio.path.clone();
    if let Some(pos) = path.rfind('.') {
        path.truncate(pos);
    }
    path.push_str(".lrc");
    (
        Some(StorageEntryLoc {
            path,
            storage_id: audio.storage_id,
        }),
        true,
    )
}

/// DB-only music fetch. The lyric arrives as a [`LyricLoadState::Loading`]
/// placeholder (loc resolved, `data` empty) — the bytes are fetched
/// separately via [`load_music_lyric`]. `music.get` used to await the
/// lyric's network round trip inline, which gated track switches (and
/// the old track's stop) on storage-plugin latency.
pub(crate) async fn get_music(cx: &BackendContext, id: MusicId) -> BResult<Option<Music>> {
    let model = cx.database_server().load_music(id).await?;
    let Some(model) = model else {
        return Ok(None);
    };

    let meta = build_music_meta(model.clone());
    let cover = if model.cover.is_some() {
        Some(DataSourceKey::Cover { id: model.id })
    } else {
        Default::default()
    };
    let lyric = resolve_lyric_loc(&model).0.map(|loc| MusicLyric {
        loc,
        data: Default::default(),
        loaded_state: LyricLoadState::Loading,
    });
    let loc = model.loc;

    Ok(Some(Music {
        meta,
        loc,
        cover,
        lyric,
    }))
}

/// Fetch + parse the lyric for a music over the storage seam — the
/// network-bound half that [`get_music`] no longer performs inline. The
/// player calls it right after the (instant) track switch and patches the
/// result into the current music. Returns `Ok(None)` when the music (or
/// its resolved lyric location) doesn't exist.
pub(crate) async fn load_music_lyric(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<MusicLyric>> {
    let model = cx.database_server().load_music(id).await?;
    let Some(model) = model else {
        return Ok(None);
    };
    let (lyric_loc, using_fallback) = resolve_lyric_loc(&model);
    Ok(load_lyric(cx, lyric_loc, using_fallback).await)
}

pub(crate) async fn get_music_abstract(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<MusicAbstract>> {
    let model = cx.database_server().load_music(id).await?;
    if model.is_none() {
        return Ok(None);
    }

    let model = model.unwrap();
    let meta = build_music_meta(model.clone());
    let cover = if model.cover.is_none() {
        Default::default()
    } else {
        Some(DataSourceKey::Cover { id: model.id })
    };

    let abstract_music = MusicAbstract { cover, meta };
    Ok(Some(abstract_music))
}
