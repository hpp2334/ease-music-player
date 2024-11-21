use std::sync::Arc;

use ease_client_shared::backends::{
    music::MusicId,
    playlist::{
        ArgAddMusicsToPlaylist, ArgCreatePlaylist, ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist,
        Playlist, PlaylistAbstract, PlaylistId,
    },
};
use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
    repositories::{
        core::get_conn,
        music::{db_add_music, ArgDBAddMusic},
        playlist::{
            db_batch_add_music_to_playlist, db_load_first_music_covers, db_load_playlists,
            db_remove_all_musics_in_playlist, db_remove_music_from_playlist, db_remove_playlist,
            db_upsert_playlist, ArgDBUpsertPlaylist,
        },
    },
    services::{
        player::player_refresh_current,
        playlist::{
            build_playlist_abstract, get_all_playlist_abstracts, get_playlist,
            notify_all_playlist_abstracts, notify_playlist,
        },
        server::loc::get_serve_url_from_music_id,
        storage::notify_storages,
    },
};

pub(crate) async fn cr_get_playlist(
    cx: &Arc<BackendContext>,
    arg: PlaylistId,
) -> BResult<Option<Playlist>> {
    get_playlist(cx, arg).await
}

pub(crate) async fn cu_update_playlist(
    cx: &Arc<BackendContext>,
    arg: ArgUpdatePlaylist,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    let current_time_ms = cx.current_time().as_millis() as i64;
    let id = arg.id;
    let arg: ArgDBUpsertPlaylist = ArgDBUpsertPlaylist {
        id: Some(id),
        title: arg.title,
        picture: arg.cover.clone(),
    };
    db_upsert_playlist(conn.get_ref(), arg, current_time_ms)?;

    try_join! {
        notify_playlist(cx, id),
        notify_all_playlist_abstracts(cx),
    }?;

    Ok(())
}

pub(crate) async fn cc_create_playlist(
    cx: &Arc<BackendContext>,
    arg: ArgCreatePlaylist,
) -> BResult<PlaylistId> {
    let cx = cx.clone();
    let mut conn = get_conn(&cx)?;
    let current_time_ms = cx.current_time().as_millis() as i64;

    let (arg, entries) = {
        let entries = arg.entries;
        let arg: ArgDBUpsertPlaylist = ArgDBUpsertPlaylist {
            id: None,
            title: arg.title,
            picture: arg.cover.clone(),
        };

        (arg, entries)
    };

    let mut musics: Vec<(MusicId, PlaylistId)> = Default::default();
    let playlist_id = conn.transaction::<PlaylistId, BError>(|conn| {
        let playlist_id = db_upsert_playlist(conn, arg, current_time_ms)?;

        for (entry, name) in entries {
            let music_id = db_add_music(
                conn,
                ArgDBAddMusic {
                    storage_id: entry.storage_id,
                    path: entry.path,
                    title: name,
                },
            )?;
            musics.push((music_id, playlist_id));
        }
        db_batch_add_music_to_playlist(conn, musics.clone())?;
        Ok(playlist_id)
    })?;

    let music_ids: Vec<MusicId> = musics.into_iter().map(|v| v.0).collect();
    {
        let cx = cx.clone();
        cx.async_runtime()
            .clone()
            .spawn_on_main(async move {
                for id in music_ids {
                    cx.player_delegate()
                        .request_total_duration(id, get_serve_url_from_music_id(&cx, id));
                }
            })
            .await;
    }

    try_join! {
        notify_all_playlist_abstracts(&cx),
    }?;

    Ok(playlist_id)
}

pub(crate) async fn cu_add_musics_to_playlist(
    cx: &Arc<BackendContext>,
    arg: ArgAddMusicsToPlaylist,
) -> BResult<()> {
    let mut conn = get_conn(&cx)?;
    let playlist_id = arg.id;

    let musics = conn.transaction(move |conn| {
        let mut musics: Vec<(MusicId, PlaylistId)> = Default::default();
        for (entry, name) in arg.entries {
            let music_id = db_add_music(
                conn,
                ArgDBAddMusic {
                    storage_id: entry.storage_id,
                    path: entry.path,
                    title: name,
                },
            )?;
            musics.push((music_id, playlist_id));
        }
        db_batch_add_music_to_playlist(conn, musics.clone())?;
        Ok::<_, BError>(musics)
    })?;

    let music_ids: Vec<MusicId> = musics.into_iter().map(|v| v.0).collect();
    {
        let cx = cx.clone();
        cx.async_runtime()
            .clone()
            .spawn_on_main(async move {
                for id in music_ids {
                    cx.player_delegate()
                        .request_total_duration(id, get_serve_url_from_music_id(&cx, id));
                }
            })
            .await;
    }

    player_refresh_current(cx).await?;

    try_join! {
        notify_playlist(cx, playlist_id),
        notify_all_playlist_abstracts(cx),
        notify_storages(cx)
    }?;

    Ok(())
}

pub(crate) async fn cd_remove_music_from_playlist(
    cx: &Arc<BackendContext>,
    arg: ArgRemoveMusicFromPlaylist,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_remove_music_from_playlist(conn.get_ref(), arg.playlist_id, arg.music_id)?;
    player_refresh_current(cx).await?;

    try_join! {
        notify_playlist(cx, arg.playlist_id),
        notify_all_playlist_abstracts(cx),
        notify_storages(cx),
    }?;

    Ok(())
}

pub(crate) async fn cd_remove_playlist(cx: &Arc<BackendContext>, arg: PlaylistId) -> BResult<()> {
    let conn = get_conn(&cx)?;

    db_remove_all_musics_in_playlist(conn.get_ref(), arg)?;
    db_remove_playlist(conn.get_ref(), arg)?;

    player_refresh_current(cx).await?;

    try_join! {
        notify_all_playlist_abstracts(cx),
    }?;

    Ok(())
}
