use std::sync::Arc;

use ease_client_shared::backends::storage::{
    ArgUpsertStorage, ListStorageEntryChildrenResp, Storage, StorageConnectionTestResult,
    StorageEntry, StorageEntryLoc, StorageId,
};
use ease_remote_storage::OneDriveBackend;
use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::BResult,
    services::{
        playlist::notify_all_playlist_abstracts,
        storage::{
            build_storage, build_storage_backend_by_arg, evict_storage_backend_cache,
            get_storage_backend, notify_storages,
        },
    },
};

pub async fn ccu_upsert_storage(cx: &Arc<BackendContext>, arg: ArgUpsertStorage) -> BResult<()> {
    let id = cx.database_server().upsert_storage(arg)?;
    evict_storage_backend_cache(cx, id);

    try_join! {
        notify_storages(cx),
    }?;
    Ok(())
}

pub async fn cr_get_refresh_token(_cx: &Arc<BackendContext>, code: String) -> BResult<String> {
    let refresh_token = OneDriveBackend::request_refresh_token(code).await?;
    Ok(refresh_token)
}

pub async fn cd_remove_storage(cx: &Arc<BackendContext>, id: StorageId) -> BResult<()> {
    cx.database_server().remove_storage(id)?;
    evict_storage_backend_cache(cx, id);

    try_join! {
        notify_storages(cx),
        notify_all_playlist_abstracts(cx),
    }?;

    Ok(())
}

pub async fn cr_test_storage(
    cx: &Arc<BackendContext>,
    arg: ArgUpsertStorage,
) -> BResult<StorageConnectionTestResult> {
    let backend = build_storage_backend_by_arg(&cx, arg)?;
    let res = backend.list("/".to_string()).await;

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
    cx: &Arc<BackendContext>,
    arg: StorageEntryLoc,
) -> BResult<ListStorageEntryChildrenResp> {
    let backend = get_storage_backend(&cx, arg.storage_id)?;
    if backend.is_none() {
        return Ok(ListStorageEntryChildrenResp::Unknown);
    }
    let backend = backend.unwrap();

    let p = arg.path;
    let res = backend.list(p).await;

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
