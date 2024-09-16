use std::{collections::HashMap, time::Duration};

use ease_client_shared::backends::{
    music::MusicId,
    playlist::{
        ArgAddMusicsToPlaylist, ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist, Playlist,
        PlaylistId, PlaylistMeta,
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    ctx::Context,
    models::playlist::PlaylistModel,
    repositories::{
        core::get_conn,
        music::{db_add_music, db_load_music_metas_by_playlist_id, ArgDBAddMusic},
        playlist::{
            db_batch_add_music_to_playlist, db_load_first_music_covers, db_load_playlists,
            db_remove_music_from_playlist, db_remove_playlist, db_upsert_playlist,
            ArgDBUpsertPlaylist, FirstMusicCovers,
        },
    },
};

use super::{music::build_music_meta, storage::to_opt_storage_entry};

fn build_playlist_meta(model: PlaylistModel, first_covers: &FirstMusicCovers) -> PlaylistMeta {
    let cover_loc =
        if let Some(picture) = to_opt_storage_entry(model.picture_path, model.picture_storage_id) {
            Some(picture)
        } else {
            let loc = first_covers
                .get(&model.id)
                .map(|c| c.clone())
                .unwrap_or_default();
            to_opt_storage_entry(loc.0, loc.1)
        };
    PlaylistMeta {
        id: model.id,
        title: model.title,
        cover_loc,
    }
}

pub(crate) async fn cr_get_all_playlist_metas(
    cx: Context,
    _arg: (),
) -> anyhow::Result<Vec<PlaylistMeta>> {
    let conn = get_conn(&cx)?;
    let models = db_load_playlists(conn.get_ref())?;
    let first_covers = db_load_first_music_covers(conn.get_ref())?;

    let ret: Vec<PlaylistMeta> = models
        .into_iter()
        .map(|model| build_playlist_meta(model, &first_covers))
        .collect();
    Ok(ret)
}

pub(crate) async fn cr_get_playlist(
    cx: Context,
    arg: PlaylistId,
) -> anyhow::Result<Option<Playlist>> {
    let conn = get_conn(&cx)?;
    let models = db_load_playlists(conn.get_ref())?;
    let first_covers = db_load_first_music_covers(conn.get_ref())?;

    let model = models.into_iter().find(|m| m.id == arg);
    if model.is_none() {
        return Ok(None);
    }
    let model = model.unwrap();
    let meta = build_playlist_meta(model, &first_covers);

    let musics = db_load_music_metas_by_playlist_id(conn.get_ref(), arg)?;
    let musics = musics.into_iter().map(|v| build_music_meta(v)).collect();

    Ok(Some(Playlist { meta, musics }))
}

pub(crate) async fn ccu_upsert_playlist(cx: Context, arg: ArgUpdatePlaylist) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let current_time_ms = arg.current_time_ms;
    let arg: ArgDBUpsertPlaylist = ArgDBUpsertPlaylist {
        id: Some(arg.id),
        title: arg.title,
        picture: arg.picture.map(|v| (v.storage_id, v.path)),
    };
    db_upsert_playlist(conn.get_ref(), arg, current_time_ms)?;
    Ok(())
}

pub(crate) async fn cu_add_musics_to_playlist(
    cx: Context,
    arg: ArgAddMusicsToPlaylist,
) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let playlist_id = arg.id;
    let mut musics: Vec<(MusicId, PlaylistId)> = Default::default();

    for entry in arg.entries {
        let music_id = db_add_music(
            conn.get_ref(),
            ArgDBAddMusic {
                storage_id: entry.0.storage_id,
                path: entry.0.path,
                title: entry.1,
            },
        )?;
        musics.push((music_id, playlist_id));
    }
    db_batch_add_music_to_playlist(conn.get_ref(), musics)?;

    Ok(())
}

pub(crate) async fn cd_remove_music_from_playlist(
    cx: Context,
    arg: ArgRemoveMusicFromPlaylist,
) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    db_remove_music_from_playlist(conn.get_ref(), arg.playlist_id, arg.music_id)?;
    Ok(())
}

pub(crate) async fn cd_remove_playlist(cx: Context, arg: PlaylistId) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let musics = db_load_music_metas_by_playlist_id(conn.get_ref(), arg)?;

    for music in musics {
        db_remove_music_from_playlist(conn.get_ref(), arg, music.id)?;
    }
    db_remove_playlist(conn.get_ref(), arg)?;

    Ok(())
}
