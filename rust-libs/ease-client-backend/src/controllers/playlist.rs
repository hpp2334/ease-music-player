use ease_client_schema::{MusicId, PlaylistId, StorageEntryLoc};
use ease_client_tokio::tokio_runtime;
use ease_order_key::{OrderKey, OrderKeyRef};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

pub async fn ct_get_playlist(cx: Arc<Backend>, arg: PlaylistId) -> BResult<Option<Playlist>> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            get_playlist(cx, arg).await
        })
        .await
        .unwrap()
}

pub async fn ct_update_playlist(cx: Arc<Backend>, arg: ArgUpdatePlaylist) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .update_playlist(arg.id, arg.title, arg.cover)
                .await?;
            Ok(())
        })
        .await
        .unwrap()
}

pub async fn ct_list_playlist(cx: Arc<Backend>) -> BResult<Vec<PlaylistAbstract>> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            get_all_playlist_abstracts(cx).await
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetCreatePlaylist {
    pub id: PlaylistId,
    pub music_ids: Vec<AddedMusic>,
}

pub async fn ct_create_playlist(
    cx: Arc<Backend>,
    arg: ArgCreatePlaylist,
) -> BResult<RetCreatePlaylist> {
    tokio_runtime()
        .handle()
        .spawn(async move {
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

            let last_order = get_all_playlist_abstracts(cx)
                .await?
                .last()
                .map(|v| OrderKey::wrap(v.meta.order.clone()))
                .unwrap_or_default();

            let (playlist_id, music_ids) = cx
                .database_server()
                .create_playlist(
                    arg.title,
                    arg.cover.clone(),
                    musics,
                    current_time_ms,
                    OrderKey::greater(&last_order),
                )
                .await?;

            Ok(RetCreatePlaylist {
                id: playlist_id,
                music_ids,
            })
        })
        .await
        .unwrap()
}

pub async fn ct_add_musics_to_playlist(
    cx: Arc<Backend>,
    arg: ArgAddMusicsToPlaylist,
) -> BResult<Vec<AddedMusic>> {
    tokio_runtime()
        .handle()
        .spawn(async move {
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

            let Some(playlist) = get_playlist(cx, arg.id).await? else {
                return Err(BError::PlaylistNotFound(arg.id));
            };
            let last_order = playlist
                .musics
                .last()
                .map(|v| OrderKey::wrap(v.meta.order.clone()))
                .unwrap_or(OrderKey::default());

            let ret = cx
                .database_server()
                .add_musics_to_playlist(arg.id, musics, last_order)
                .await?;

            Ok(ret)
        })
        .await
        .unwrap()
}

pub async fn ct_remove_music_from_playlist(
    cx: Arc<Backend>,
    arg: ArgRemoveMusicFromPlaylist,
) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .remove_music_from_playlist(arg.playlist_id, arg.music_id)
                .await?;
            Ok(())
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgReorderPlaylist {
    pub id: PlaylistId,
    pub a: Option<PlaylistId>,
    pub b: Option<PlaylistId>,
}

pub fn cts_reorder_playlist(cx: Arc<Backend>, arg: ArgReorderPlaylist) -> BResult<()> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { reorder_playlist_inner(&cx, arg).await })
}

pub(crate) async fn reorder_playlist_inner(
    cx: &BackendContext,
    arg: ArgReorderPlaylist,
) -> BResult<()> {
    if arg.a == arg.b {
        return Ok(());
    }

    let playlists = get_all_playlist_abstracts(cx).await?;

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
        .set_playlist_order(from.meta.id, order)
        .await?;
    Ok(())
}

pub async fn ct_remove_playlist(cx: Arc<Backend>, arg: PlaylistId) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server().remove_playlist(arg).await?;
            Ok(())
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgReorderMusic {
    pub playlist_id: PlaylistId,
    pub id: MusicId,
    pub a: Option<MusicId>,
    pub b: Option<MusicId>,
}

pub fn cts_reorder_music_in_playlist(cx: Arc<Backend>, arg: ArgReorderMusic) -> BResult<()> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { reorder_music_in_playlist_inner(&cx, arg).await })
}

pub(crate) async fn reorder_music_in_playlist_inner(
    cx: &BackendContext,
    arg: ArgReorderMusic,
) -> BResult<()> {
    if arg.a == arg.b {
        return Ok(());
    }
    let Some(playlist) = get_playlist(cx, arg.playlist_id).await? else {
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

    cx.database_server()
        .set_music_order(from.meta.id, order)
        .await?;
    Ok(())
}
