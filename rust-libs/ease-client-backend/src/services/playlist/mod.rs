use std::time::Duration;

use ease_client_schema::{DataSourceKey, MusicId, PlaylistId, PlaylistModel, StorageId};

use crate::{
    ctx::BackendContext,
    error::BResult,
    objects::{MusicAbstract, Playlist, PlaylistAbstract, PlaylistMeta},
};

use super::music::build_music_abstract;

fn compute_musics_duration(list: &Vec<MusicAbstract>) -> Option<Duration> {
    let mut sum: Duration = Default::default();
    for v in list {
        if let Some(v) = v.meta.duration {
            sum += v;
        } else {
            return None;
        }
    }
    Some(sum)
}

pub(crate) fn build_playlist_meta(
    _cx: &BackendContext,
    model: PlaylistModel,
    first_cover_music_id: Option<MusicId>,
) -> PlaylistMeta {
    let cover_loc = model.picture;
    let show_cover = if let Some(loc) = cover_loc.clone() {
        Some(DataSourceKey::AnyEntry { entry: loc })
    } else {
        let id = first_cover_music_id;
        if let Some(id) = id {
            Some(DataSourceKey::Cover { id })
        } else {
            Default::default()
        }
    };
    PlaylistMeta {
        id: model.id,
        title: model.title,
        cover: cover_loc,
        show_cover,
        created_time: Duration::from_millis(model.created_time as u64),
        order: model.order,
        storage_allowlist: model.storage_allowlist,
    }
}

pub(crate) async fn build_playlist_abstract(
    cx: &BackendContext,
    model: PlaylistModel,
) -> BResult<(PlaylistAbstract, Vec<MusicAbstract>)> {
    let id = model.id;
    let musics = cx.database_server().load_musics_by_playlist_id(id).await?;
    let first_cover_music_id = musics.iter().find(|m| m.cover.is_some()).map(|v| v.id);
    // Distinct storages the playlist's musics live on — feeds the edit
    // dialog's rule that referenced storages can't be unchecked from
    // the allowlist. Sorted + deduped for deterministic output.
    let mut music_storage_ids: Vec<StorageId> = musics.iter().map(|m| m.loc.storage_id).collect();
    music_storage_ids.sort_unstable();
    music_storage_ids.dedup();
    let meta = build_playlist_meta(cx, model, first_cover_music_id);

    let musics = musics
        .into_iter()
        .map(|v| build_music_abstract(cx, v))
        .collect();
    let duration = compute_musics_duration(&musics);

    let abstr = PlaylistAbstract {
        meta,
        music_count: musics.len() as u64,
        music_storage_ids,
        duration,
    };

    Ok((abstr, musics))
}

pub async fn get_playlist(cx: &BackendContext, arg: PlaylistId) -> BResult<Option<Playlist>> {
    let model = cx.database_server().load_playlist(arg).await?;

    if model.is_none() {
        return Ok(None);
    }
    let model = model.unwrap();
    let (abstr, musics) = build_playlist_abstract(cx, model).await?;

    Ok(Some(Playlist { abstr, musics }))
}

pub(crate) async fn get_all_playlist_abstracts(
    cx: &BackendContext,
) -> BResult<Vec<PlaylistAbstract>> {
    let models = cx.database_server().load_playlists().await?;

    let mut ret: Vec<PlaylistAbstract> = Default::default();
    for model in models {
        let (abstr, _) = build_playlist_abstract(cx, model).await?;
        ret.push(abstr)
    }

    Ok(ret)
}
