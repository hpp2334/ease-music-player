use std::sync::Arc;

use ease_client_shared::backends::{
    music::MusicId,
    playlist::{
        ArgAddMusicsToPlaylist, ArgCreatePlaylist, ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist,
        Playlist, PlaylistId,
    },
    storage::StorageEntryLoc,
};
use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
    repositories::music::ArgDBAddMusic,
    services::{
        player::player_refresh_current,
        playlist::{get_playlist, notify_all_playlist_abstracts, notify_playlist},
        storage::notify_storages,
    },
};

pub(crate) async fn cr_get_playlist(
    cx: &BackendContext,
    arg: PlaylistId,
) -> BResult<Option<Playlist>> {
    get_playlist(cx, arg).await
}

pub(crate) async fn cu_update_playlist(cx: &BackendContext, arg: ArgUpdatePlaylist) -> BResult<()> {
    let current_time_ms = cx.current_time().as_millis() as i64;

    cx.database_server()
        .update_playlist(arg.id, arg.title, arg.cover, current_time_ms)?;

    try_join! {
        notify_playlist(cx, arg.id),
        notify_all_playlist_abstracts(cx),
    }?;

    Ok(())
}

pub(crate) async fn cc_create_playlist(
    cx: &BackendContext,
    arg: ArgCreatePlaylist,
) -> BResult<PlaylistId> {
    let current_time_ms = cx.current_time().as_millis() as i64;

    let musics = arg
        .entries
        .clone()
        .into_iter()
        .map(|(entry, name)| ArgDBAddMusic {
            loc: StorageEntryLoc {
                storage_id: entry.storage_id,
                path: entry.path,
            },
            title: name,
        })
        .collect();

    let (playlist_id, music_ids) = cx.database_server().create_playlist(
        arg.title,
        arg.cover.clone(),
        musics,
        current_time_ms,
    )?;

    {
        let rt = cx.async_runtime().clone();
        let cx = cx.weak();
        rt.clone()
            .clone()
            .spawn_on_main(async move {
                if let Some(cx) = cx.upgrade() {
                    for id in music_ids {
                        cx.player_delegate()
                            .request_total_duration(id, cx.asset_server().serve_music_url(id));
                    }
                }
            })
            .await;
    }

    try_join! {
        notify_all_playlist_abstracts(&cx),
        notify_storages(&cx),
    }?;

    Ok(playlist_id)
}

pub(crate) async fn cu_add_musics_to_playlist(
    cx: &BackendContext,
    arg: ArgAddMusicsToPlaylist,
) -> BResult<()> {
    let playlist_id = arg.id;
    let musics = arg
        .entries
        .clone()
        .into_iter()
        .map(|(entry, name)| ArgDBAddMusic {
            loc: StorageEntryLoc {
                storage_id: entry.storage_id,
                path: entry.path,
            },
            title: name,
        })
        .collect();

    let music_ids = cx.database_server().add_musics_to_playlist(musics)?;

    {
        let rt = cx.async_runtime().clone();
        let cx = cx.weak();
        rt.clone()
            .clone()
            .spawn_on_main(async move {
                if let Some(cx) = cx.upgrade() {
                    for id in music_ids {
                        cx.player_delegate()
                            .request_total_duration(id, cx.asset_server().serve_music_url(id));
                    }
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
    cx: &BackendContext,
    arg: ArgRemoveMusicFromPlaylist,
) -> BResult<()> {
    cx.database_server()
        .remove_music_from_playlist(arg.playlist_id, arg.music_id)?;
    player_refresh_current(cx).await?;

    try_join! {
        notify_playlist(cx, arg.playlist_id),
        notify_all_playlist_abstracts(cx),
        notify_storages(cx),
    }?;

    Ok(())
}

pub(crate) async fn cd_remove_playlist(cx: &BackendContext, arg: PlaylistId) -> BResult<()> {
    cx.database_server().remove_playlist(arg)?;

    player_refresh_current(cx).await?;

    try_join! {
        notify_all_playlist_abstracts(cx),
    }?;

    Ok(())
}
