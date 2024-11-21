use ease_client_shared::backends::storage::{
    ArgUpsertStorage, ListStorageEntryChildrenResp, Storage, StorageConnectionTestResult,
    StorageEntry, StorageEntryLoc, StorageId, StorageType,
};
use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::BResult,
    repositories::{
        core::get_conn,
        music::db_get_playlists_count_by_storage,
        playlist::{db_get_musics_count_by_storage, db_remove_musics_in_playlists_by_storage},
        storage::{db_load_storage, db_load_storages, db_remove_storage, db_upsert_storage},
    },
    services::{
        playlist::notify_all_playlist_abstracts,
        storage::{
            build_storage, get_storage_backend, get_storage_backend_by_arg, list_storage,
            notify_storages,
        },
    },
};

pub async fn ccu_upsert_storage(cx: &BackendContext, arg: ArgUpsertStorage) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_upsert_storage(conn.get_ref(), arg)?;

    try_join! {
        notify_storages(cx),
    }?;
    Ok(())
}

pub async fn cr_list_storage(cx: &BackendContext, _arg: ()) -> BResult<Vec<Storage>> {
    list_storage(cx).await
}

pub async fn cr_get_storage(cx: &BackendContext, id: StorageId) -> BResult<Option<Storage>> {
    let conn = get_conn(&cx)?;
    let model = db_load_storage(conn.get_ref(), id)?;
    let storage = if let Some(model) = model {
        let music_count = db_get_musics_count_by_storage(conn.get_ref(), id)?;
        let playlist_count = db_get_playlists_count_by_storage(conn.get_ref(), id)?;
        Some(build_storage(model, music_count, playlist_count))
    } else {
        None
    };
    Ok(storage)
}

pub async fn cd_remove_storage(cx: &BackendContext, id: StorageId) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_remove_musics_in_playlists_by_storage(conn.get_ref(), id)?;
    db_remove_storage(conn.get_ref(), id)?;

    try_join! {
        notify_storages(cx),
        notify_all_playlist_abstracts(cx),
    }?;

    Ok(())
}

pub async fn cr_test_storage(
    cx: &BackendContext,
    arg: ArgUpsertStorage,
) -> BResult<StorageConnectionTestResult> {
    let backend = get_storage_backend_by_arg(&cx, arg)?;
    let res = backend.get("/").await;

    match res {
        Ok(_) => Ok(StorageConnectionTestResult::Success),
        Err(e) => {
            if e.is_unauthorized() {
                Ok(StorageConnectionTestResult::Unauthorized)
            } else if e.is_timeout() {
                Ok(StorageConnectionTestResult::Timeout)
            } else {
                Ok(StorageConnectionTestResult::OtherError)
            }
        }
    }
}

pub async fn cr_list_storage_entry_children(
    cx: &BackendContext,
    arg: StorageEntryLoc,
) -> BResult<ListStorageEntryChildrenResp> {
    let backend = get_storage_backend(&cx, arg.storage_id)?;
    if backend.is_none() {
        return Ok(ListStorageEntryChildrenResp::Unknown);
    }
    let backend = backend.unwrap();

    let p = arg.path.as_str();
    let res = backend.list(&p).await;

    match res {
        Ok(entries) => {
            let entries = entries
                .into_iter()
                .map(|entry| StorageEntry {
                    storage_id: arg.storage_id,
                    name: entry.name,
                    path: entry.path,
                    size: entry.size,
                    is_dir: entry.is_dir,
                })
                .collect();
            Ok(ListStorageEntryChildrenResp::Ok(entries))
        }
        Err(e) => {
            tracing::error!("{}", e);
            if e.is_unauthorized() {
                Ok(ListStorageEntryChildrenResp::AuthenticationFailed)
            } else if e.is_timeout() {
                Ok(ListStorageEntryChildrenResp::Timeout)
            } else {
                Ok(ListStorageEntryChildrenResp::Unknown)
            }
        }
    }
}
