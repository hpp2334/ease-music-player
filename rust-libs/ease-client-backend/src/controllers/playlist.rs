use std::time::Duration;

use ease_client_shared::backends::{
    music::{MusicId, MusicMeta},
    music_duration::MusicDuration,
    playlist::{
        ArgAddMusicsToPlaylist, ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist, Playlist,
        PlaylistAbstract, PlaylistId, PlaylistMeta,
    },
};
use ease_database::DbConnectionRef;

use crate::{
    ctx::BackendGlobal,
    error::BResult,
    models::playlist::PlaylistModel,
    repositories::{
        core::get_conn,
        music::{db_add_music, db_load_music_metas_by_playlist_id, ArgDBAddMusic},
        playlist::{
            db_batch_add_music_to_playlist, db_load_first_music_covers,
            db_load_playlist_music_count, db_load_playlists, db_remove_music_from_playlist,
            db_remove_playlist, db_upsert_playlist, ArgDBUpsertPlaylist, FirstMusicCovers,
        },
    },
    services::{music::build_music_meta, server::loc::get_serve_url_from_opt_loc},
};

use super::storage::to_opt_storage_entry;

fn build_playlist_meta(
    cx: &BackendGlobal,
    model: PlaylistModel,
    first_covers: &FirstMusicCovers,
) -> PlaylistMeta {
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
        cover_url: get_serve_url_from_opt_loc(cx, cover_loc),
        created_time: Duration::from_millis(model.created_time as u64),
    }
}

fn compute_musics_duration(list: &Vec<MusicMeta>) -> Option<MusicDuration> {
    let mut sum: Duration = Default::default();
    for v in list {
        if let Some(v) = v.duration {
            sum += *v;
        } else {
            return None;
        }
    }
    Some(MusicDuration::new(sum))
}

fn build_playlist_abstract(
    cx: &BackendGlobal,
    conn: DbConnectionRef,
    model: PlaylistModel,
    first_covers: &FirstMusicCovers,
) -> BResult<(PlaylistAbstract, Vec<MusicMeta>)> {
    let id = model.id;
    let meta = build_playlist_meta(&cx, model, &first_covers);
    let musics = db_load_music_metas_by_playlist_id(conn, id)?;
    let musics = musics.into_iter().map(|v| build_music_meta(v)).collect();
    let duration = compute_musics_duration(&musics);

    let abstr = PlaylistAbstract {
        meta,
        music_count: musics.len(),
        duration,
    };

    Ok((abstr, musics))
}

pub(crate) async fn cr_get_all_playlist_abstracts(
    cx: BackendGlobal,
    _arg: (),
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

pub(crate) async fn cr_get_playlist(cx: BackendGlobal, arg: PlaylistId) -> BResult<Option<Playlist>> {
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
}

pub(crate) async fn ccu_upsert_playlist(cx: BackendGlobal, arg: ArgUpdatePlaylist) -> BResult<()> {
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
    cx: BackendGlobal,
    arg: ArgAddMusicsToPlaylist,
) -> BResult<()> {
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
    cx: BackendGlobal,
    arg: ArgRemoveMusicFromPlaylist,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_remove_music_from_playlist(conn.get_ref(), arg.playlist_id, arg.music_id)?;
    Ok(())
}

pub(crate) async fn cd_remove_playlist(cx: BackendGlobal, arg: PlaylistId) -> BResult<()> {
    let conn = get_conn(&cx)?;
    let musics = db_load_music_metas_by_playlist_id(conn.get_ref(), arg)?;

    for music in musics {
        db_remove_music_from_playlist(conn.get_ref(), arg, music.id)?;
    }
    db_remove_playlist(conn.get_ref(), arg)?;

    Ok(())
}
