use std::time::Duration;

use ease_client_schema::{
    DataSourceKey, MusicId, MusicModel, PlaylistId, StorageEntryLoc, StorageId,
};
use serde::{Deserialize, Serialize};

use crate::{
    ctx::BackendContext,
    error::BResult,
    objects::{LyricLoadState, Music, MusicAbstract, MusicLyric, MusicMeta},
    StorageEntry,
};

use super::{lyrics::parse_lyric_content, storage::load_storage_entry_data};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdatePlaylist {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    /// Import-source restriction (`None` = all storages). Defaults for
    /// older callers that predate the field.
    #[serde(default)]
    pub storage_allowlist: Option<Vec<StorageId>>,
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
    /// Import-source restriction (`None` = all storages). Defaults for
    /// older callers that predate the field.
    #[serde(default)]
    pub storage_allowlist: Option<Vec<StorageId>>,
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

/// Try each candidate location in order: a fetch miss (missing file /
/// mid-stream error — e.g. a missing sibling) moves on to the next
/// candidate. The first successfully fetched file is parsed exactly once
/// through the plugin chain ([`parse_lyric_content`]) — a parse failure
/// is terminal (`Failed`), it does not retry further locations (the file
/// exists but nothing can parse it). All candidates missing ⇒ `Missing`
/// when every candidate was a fallback, `Failed` for the explicit pick.
async fn load_lyric(
    cx: &BackendContext,
    candidates: Vec<(StorageEntryLoc, bool)>,
) -> Option<MusicLyric> {
    if candidates.is_empty() {
        return None;
    }
    for (loc, _) in candidates.iter() {
        let data = load_storage_entry_data(cx, loc).await;
        let bytes = match data {
            Err(e) => {
                tracing::error!("fail to load entry {loc:?}: {e}");
                continue;
            }
            Ok(None) => continue,
            Ok(Some(bytes)) => bytes,
        };
        let file_name = loc
            .path
            .rsplit('/')
            .next()
            .unwrap_or(loc.path.as_str())
            .to_string();
        return Some(match parse_lyric_content(cx, &file_name, &bytes).await {
            Ok(data) => MusicLyric {
                loc: loc.clone(),
                data,
                loaded_state: LyricLoadState::Loaded,
            },
            Err(e) => {
                tracing::error!("fail to parse lyric '{file_name}': {e}");
                MusicLyric {
                    loc: loc.clone(),
                    data: Default::default(),
                    loaded_state: LyricLoadState::Failed,
                }
            }
        });
    }
    // Every candidate missed.
    let (loc, is_fallback) = candidates.into_iter().next().unwrap();
    Some(MusicLyric {
        loc,
        data: Default::default(),
        loaded_state: if is_fallback {
            LyricLoadState::Missing
        } else {
            LyricLoadState::Failed
        },
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

/// Resolve a music's candidate lyric locations: the explicit `model.lyric`
/// if set, else — when `lyric_default` is enabled — one sibling per
/// registered parser extension (`<audio-base>.<ext>`, registry order —
/// `plugin_manager::PluginManagerShared::lyric_sibling_extensions`).
/// Empty when nothing can parse (no parser plugin enabled and no explicit
/// pick). Returns `(loc, is_fallback)` pairs; only the explicit pick is
/// ever non-fallback.
fn resolve_lyric_locs(
    model: &MusicModel,
    sibling_extensions: &[String],
) -> Vec<(StorageEntryLoc, bool)> {
    if let Some(loc) = model.lyric.clone() {
        return vec![(loc, false)];
    }
    if !model.lyric_default || sibling_extensions.is_empty() {
        return Vec::new();
    }
    let audio = &model.loc;
    let mut base = audio.path.clone();
    if let Some(pos) = base.rfind('.') {
        base.truncate(pos);
    }
    sibling_extensions
        .iter()
        .map(|ext| {
            (
                StorageEntryLoc {
                    path: format!("{base}.{ext}"),
                    storage_id: audio.storage_id,
                },
                true,
            )
        })
        .collect()
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
    let lyric = resolve_lyric_locs(&model, &cx.plugin_manager().lyric_sibling_extensions())
        .into_iter()
        .next()
        .map(|(loc, _)| MusicLyric {
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
    let candidates = resolve_lyric_locs(&model, &cx.plugin_manager().lyric_sibling_extensions());
    Ok(load_lyric(cx, candidates).await)
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
