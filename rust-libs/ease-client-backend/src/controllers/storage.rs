use std::sync::Arc;

use ease_client_schema::{StorageEntryLoc, StorageId};
use ease_remote_storage::OneDriveBackend;

use crate::{
    error::BResult,
    objects::{ListStorageEntryChildrenResp, Storage, StorageConnectionTestResult, StorageEntry},
    onedrive_oauth_url,
    services::{
        build_storage_backend_by_arg, evict_storage_backend_cache, get_storage_backend,
        list_storage,
    },
    ArgUpsertStorage, Backend,
};

fn normalize_arg_upsert_storage(mut arg: ArgUpsertStorage) -> ArgUpsertStorage {
    if arg.is_anonymous {
        arg.username = Default::default();
        arg.password = Default::default();
    }
    arg
}

#[uniffi::export]
pub async fn ct_list_storage(cx: Arc<Backend>) -> BResult<Vec<Storage>> {
    let cx = cx.get_context();
    let storages = list_storage(cx).await?;

    Ok(storages)
}

#[uniffi::export]
pub async fn ct_upsert_storage(cx: Arc<Backend>, arg: ArgUpsertStorage) -> BResult<()> {
    let arg = normalize_arg_upsert_storage(arg);

    let cx = cx.get_context();
    let id = cx.database_server().upsert_storage(arg)?;
    evict_storage_backend_cache(cx, id);

    Ok(())
}

#[uniffi::export]
pub async fn ct_get_refresh_token(cx: Arc<Backend>, code: String) -> BResult<String> {
    let cx = cx.get_context();
    let refresh_token = OneDriveBackend::request_refresh_token(code).await?;
    Ok(refresh_token)
}

#[uniffi::export]
pub async fn ct_remove_storage(cx: Arc<Backend>, id: StorageId) -> BResult<()> {
    let cx = cx.get_context();
    cx.database_server().remove_storage(id)?;
    evict_storage_backend_cache(cx, id);

    Ok(())
}

#[uniffi::export]
pub async fn ct_test_storage(
    cx: Arc<Backend>,
    arg: ArgUpsertStorage,
) -> BResult<StorageConnectionTestResult> {
    let arg = normalize_arg_upsert_storage(arg);
    let cx = cx.get_context();
    let backend = build_storage_backend_by_arg(cx, arg)?;
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

#[uniffi::export]
pub async fn ct_list_storage_entry_children(
    cx: Arc<Backend>,
    arg: StorageEntryLoc,
) -> BResult<ListStorageEntryChildrenResp> {
    let cx = cx.get_context();
    let backend = get_storage_backend(cx, arg.storage_id)?;
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
                    size: entry.size.map(|s| s as u64),
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

#[uniffi::export]
pub fn ct_onedrive_oauth_url() -> String {
    onedrive_oauth_url()
}
