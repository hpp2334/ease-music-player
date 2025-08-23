use std::sync::Arc;

use ease_client_schema::{MusicId, PlaylistId, StorageEntryLoc};

use crate::{
    error::BResult,
    objects::{Playlist, PlaylistAbstract},
    repositories::{music::ArgDBAddMusic, playlist::AddedMusic},
    services::{
        get_all_playlist_abstracts, get_playlist, ArgAddMusicsToPlaylist, ArgCreatePlaylist,
        ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist,
    },
    Backend,
};

#[uniffi::export]
pub async fn ct_get_playlist(cx: Arc<Backend>, arg: PlaylistId) -> BResult<Option<Playlist>> {
    let cx = cx.get_context();
    get_playlist(cx, arg).await
}

#[uniffi::export]
pub async fn ct_update_playlist(cx: Arc<Backend>, arg: ArgUpdatePlaylist) -> BResult<()> {
    let cx = cx.get_context();
    cx.database_server()
        .update_playlist(arg.id, arg.title, arg.cover)?;

    Ok(())
}

#[uniffi::export]
pub async fn ct_list_playlist(cx: Arc<Backend>) -> BResult<Vec<PlaylistAbstract>> {
    let cx = cx.get_context();
    return get_all_playlist_abstracts(cx).await;
}

#[derive(uniffi::Record)]
pub struct RetCreatePlaylist {
    id: PlaylistId,
    music_ids: Vec<AddedMusic>,
}

#[uniffi::export]
pub async fn ct_create_playlist(
    cx: Arc<Backend>,
    arg: ArgCreatePlaylist,
) -> BResult<RetCreatePlaylist> {
    let cx = cx.get_context();
    let current_time_ms = cx.current_time().as_millis() as i64;

    let musics = arg
        .entries
        .clone()
        .into_iter()
        .map(|arg| {
            let entry = arg.entry;
            let name = arg.name;
            ArgDBAddMusic {
                loc: StorageEntryLoc {
                    storage_id: entry.storage_id,
                    path: entry.path,
                },
                title: name,
            }
        })
        .collect();

    let (playlist_id, music_ids) = cx.database_server().create_playlist(
        arg.title,
        arg.cover.clone(),
        musics,
        current_time_ms,
    )?;

    Ok(RetCreatePlaylist {
        id: playlist_id,
        music_ids,
    })
}

#[uniffi::export]
pub async fn ct_add_musics_to_playlist(
    cx: Arc<Backend>,
    arg: ArgAddMusicsToPlaylist,
) -> BResult<Vec<AddedMusic>> {
    let cx = cx.get_context();
    let musics = arg
        .entries
        .clone()
        .into_iter()
        .map(|arg| {
            let entry = arg.entry;
            let name = arg.name;
            ArgDBAddMusic {
                loc: StorageEntryLoc {
                    storage_id: entry.storage_id,
                    path: entry.path,
                },
                title: name,
            }
        })
        .collect();

    let ret = cx
        .database_server()
        .add_musics_to_playlist(arg.id, musics)?;

    Ok(ret)
}

#[uniffi::export]
pub async fn ct_remove_music_from_playlist(
    cx: Arc<Backend>,
    arg: ArgRemoveMusicFromPlaylist,
) -> BResult<()> {
    let cx = cx.get_context();
    cx.database_server()
        .remove_music_from_playlist(arg.playlist_id, arg.music_id)?;

    Ok(())
}

#[uniffi::export]
pub async fn ct_remove_playlist(cx: Arc<Backend>, arg: PlaylistId) -> BResult<()> {
    let cx = cx.get_context();
    cx.database_server().remove_playlist(arg)?;

    Ok(())
}
