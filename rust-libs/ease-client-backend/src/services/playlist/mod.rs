use std::time::Duration;

use ease_client_shared::backends::{
    connector::ConnectorAction,
    music::MusicAbstract,
    music_duration::MusicDuration,
    playlist::{Playlist, PlaylistAbstract, PlaylistId, PlaylistMeta},
    storage::DataSourceKey,
};
use ease_database::DbConnectionRef;

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::playlist::PlaylistModel,
    repositories::{
        core::get_conn,
        music::db_load_music_metas_by_playlist_id,
        playlist::{db_load_first_music_covers, db_load_playlists, FirstMusicCovers},
    },
};

use super::{
    music::build_music_abstract,
    server::loc::{get_serve_cover_url_from_music_id, get_serve_url_from_loc},
    storage::to_opt_storage_entry,
};

fn compute_musics_duration(list: &Vec<MusicAbstract>) -> Option<MusicDuration> {
    let mut sum: Duration = Default::default();
    for v in list {
        if let Some(v) = v.meta.duration {
            sum += *v;
        } else {
            return None;
        }
    }
    Some(MusicDuration::new(sum))
}

pub(crate) fn build_playlist_meta(
    cx: &BackendContext,
    model: PlaylistModel,
    first_covers: &FirstMusicCovers,
) -> PlaylistMeta {
    let cover_loc =
        if let Some(picture) = to_opt_storage_entry(model.picture_path, model.picture_storage_id) {
            Some(picture)
        } else {
            None
        };
    let show_cover = if let Some(loc) = cover_loc.clone() {
        Some(DataSourceKey::AnyEntry { entry: loc })
    } else {
        let id = first_covers.get(&model.id).map(|c| c.clone());
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
    }
}

pub(crate) fn build_playlist_abstract(
    cx: &BackendContext,
    conn: DbConnectionRef,
    model: PlaylistModel,
    first_covers: &FirstMusicCovers,
) -> BResult<(PlaylistAbstract, Vec<MusicAbstract>)> {
    let id = model.id;
    let meta = build_playlist_meta(&cx, model, &first_covers);
    let musics = db_load_music_metas_by_playlist_id(conn, id)?;
    let musics = musics
        .into_iter()
        .map(|v| build_music_abstract(cx, v))
        .collect();
    let duration = compute_musics_duration(&musics);

    let abstr = PlaylistAbstract {
        meta,
        music_count: musics.len(),
        duration,
    };

    Ok((abstr, musics))
}

pub(crate) async fn get_playlist(
    cx: &BackendContext,
    arg: PlaylistId,
) -> BResult<Option<Playlist>> {
    let rt = cx.async_runtime();
    let cx = cx.clone();
    rt.spawn(async move {
        let conn = get_conn(&cx)?;
        let models = db_load_playlists(conn.get_ref())?;
        let first_covers = db_load_first_music_covers(conn.get_ref())?;

        let model = models.into_iter().find(|m| m.id == arg);
        if model.is_none() {
            return Ok(None);
        }
        let model = model.unwrap();
        let (abstr, musics) = build_playlist_abstract(&cx, conn.get_ref(), model, &first_covers)?;

        Ok(Some(Playlist { abstr, musics }))
    })
    .await
}

pub(crate) async fn get_all_playlist_abstracts(
    cx: &BackendContext,
) -> BResult<Vec<PlaylistAbstract>> {
    let conn = get_conn(&cx)?;
    let models = db_load_playlists(conn.get_ref())?;
    let first_covers = db_load_first_music_covers(conn.get_ref())?;

    let mut ret: Vec<PlaylistAbstract> = Default::default();
    for model in models {
        let (abstr, _) = build_playlist_abstract(&cx, conn.get_ref(), model, &first_covers)?;
        ret.push(abstr)
    }

    Ok(ret)
}

pub(crate) async fn notify_all_playlist_abstracts(cx: &BackendContext) -> BResult<()> {
    let playlists = get_all_playlist_abstracts(cx).await?;
    cx.notify(ConnectorAction::PlaylistAbstracts(playlists));
    Ok(())
}

pub(crate) async fn notify_playlist(cx: &BackendContext, id: PlaylistId) -> BResult<()> {
    let playlist = get_playlist(cx, id).await?;
    if let Some(playlist) = playlist {
        cx.notify(ConnectorAction::Playlist(playlist));
    }
    Ok(())
}
