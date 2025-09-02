use std::sync::Arc;

use ease_client_schema::{MusicId, PlaylistId, StorageEntryLoc};
use ease_order_key::{OrderKey, OrderKeyRef};

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
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
    get_playlist(cx, arg)
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
    return get_all_playlist_abstracts(cx);
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

    let last_order = get_all_playlist_abstracts(cx)?
        .last()
        .map(|v| OrderKey::wrap(v.meta.order.clone()))
        .unwrap_or_default();

    let (playlist_id, music_ids) = cx.database_server().create_playlist(
        arg.title,
        arg.cover.clone(),
        musics,
        current_time_ms,
        OrderKey::greater(&last_order),
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

    let Some(playlist) = get_playlist(cx, arg.id)? else {
        return Err(BError::PlaylistNotFound(arg.id));
    };
    let last_order = playlist
        .musics
        .last()
        .map(|v| OrderKey::wrap(v.meta.order.clone()))
        .unwrap_or(OrderKey::default());

    let ret = cx
        .database_server()
        .add_musics_to_playlist(arg.id, musics, last_order)?;

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

#[derive(uniffi::Record)]
pub struct ArgReorderPlaylist {
    id: PlaylistId,
    a: Option<PlaylistId>,
    b: Option<PlaylistId>,
}

#[uniffi::export]
pub fn cts_reorder_playlist(cx: Arc<Backend>, arg: ArgReorderPlaylist) -> BResult<()> {
    let cx = cx.get_context();
    if arg.a == arg.b {
        return Ok(());
    }

    let playlists = get_all_playlist_abstracts(cx)?;

    let from = playlists
        .iter()
        .find(|v| v.meta.id == arg.id)
        .ok_or(BError::PlaylistNotFound(arg.id))?;
    let a = match arg.a {
        Some(id) => Some(
            playlists
                .iter()
                .find(|v| v.meta.id == id)
                .ok_or(BError::PlaylistNotFound(id))?,
        ),
        None => None,
    };
    let b = match arg.b {
        Some(id) => Some(
            playlists
                .iter()
                .find(|v| v.meta.id == id)
                .ok_or(BError::PlaylistNotFound(id))?,
        ),
        None => None,
    };

    if a.is_none() && b.is_none() {
        tracing::warn!("reorder but both playlists are null");
        return Ok(());
    }

    let a_order = a.map(|v| OrderKeyRef::wrap(&v.meta.order));
    let b_order = b.map(|v| OrderKeyRef::wrap(&v.meta.order));
    let order = {
        match (a_order, b_order) {
            (Some(a), Some(b)) => OrderKey::between(a, b)?,
            (Some(a), None) => OrderKey::greater(a),
            (None, Some(b)) => OrderKey::less_or_fallback(b),
            (None, None) => unreachable!(),
        }
    };

    cx.database_server()
        .set_playlist_order(from.meta.id, order)?;
    Ok(())
}

#[uniffi::export]
pub async fn ct_remove_playlist(cx: Arc<Backend>, arg: PlaylistId) -> BResult<()> {
    let cx = cx.get_context();
    cx.database_server().remove_playlist(arg)?;

    Ok(())
}

#[derive(uniffi::Record)]
pub struct ArgReorderMusic {
    playlist_id: PlaylistId,
    id: MusicId,
    a: Option<MusicId>,
    b: Option<MusicId>,
}

#[uniffi::export]
pub fn cts_reorder_music_in_playlist(cx: Arc<Backend>, arg: ArgReorderMusic) -> BResult<()> {
    let cx = cx.get_context();
    if arg.a == arg.b {
        return Ok(());
    }
    let Some(playlist) = get_playlist(cx, arg.playlist_id)? else {
        return Err(BError::PlaylistNotFound(arg.playlist_id));
    };

    let from = playlist
        .musics
        .iter()
        .find(|v| v.meta.id == arg.id)
        .ok_or(BError::MusicNotFound(arg.id))?;
    let a = match arg.a {
        Some(id) => Some(
            playlist
                .musics
                .iter()
                .find(|v| v.meta.id == id)
                .ok_or(BError::MusicNotFound(id))?,
        ),
        None => None,
    };
    let b = match arg.b {
        Some(id) => Some(
            playlist
                .musics
                .iter()
                .find(|v| v.meta.id == id)
                .ok_or(BError::MusicNotFound(id))?,
        ),
        None => None,
    };

    if a.is_none() && b.is_none() {
        tracing::warn!("reorder but both musics are null");
        return Ok(());
    }

    let a_order = a.map(|v| OrderKeyRef::wrap(&v.meta.order));
    let b_order = b.map(|v| OrderKeyRef::wrap(&v.meta.order));
    let order = {
        match (a_order, b_order) {
            (Some(a), Some(b)) => OrderKey::between(a, b)?,
            (Some(a), None) => OrderKey::greater(a),
            (None, Some(b)) => OrderKey::less_or_fallback(b),
            (None, None) => unreachable!(),
        }
    };

    cx.database_server().set_music_order(from.meta.id, order)?;
    Ok(())
}
