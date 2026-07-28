use std::sync::Arc;
use ease_client_tokio::tokio_runtime;

use ease_client_schema::{StorageEntryLoc, StorageId};
use ease_remote_storage::OneDriveBackend;

use crate::{
    error::BResult,
    objects::{ListStorageEntryChildrenResp, Storage, StorageConnectionTestResult, StorageEntry},
    onedrive_oauth_url,
    services::{
        build_storage_backend_by_arg, get_storage_backend, list_storage, remove_storage,
        upsert_storage,
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

pub async fn ct_list_storage(cx: Arc<Backend>) -> BResult<Vec<Storage>> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        let storages = list_storage(cx).await?;

        Ok(storages)
    }).await.unwrap()
}

pub async fn ct_upsert_storage(cx: Arc<Backend>, arg: ArgUpsertStorage) -> BResult<()> {
    tokio_runtime().handle().spawn(async move {
        let arg = normalize_arg_upsert_storage(arg);

        let cx = cx.get_context();
        upsert_storage(cx, arg).await?;

        Ok(())
    }).await.unwrap()
}

pub async fn ct_get_refresh_token(cx: Arc<Backend>, code: String) -> BResult<String> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        let refresh_token = OneDriveBackend::request_refresh_token(code).await?;
        Ok(refresh_token)
    }).await.unwrap()
}

pub async fn ct_remove_storage(cx: Arc<Backend>, id: StorageId) -> BResult<()> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        remove_storage(cx, id).await?;

        Ok(())
    }).await.unwrap()
}

pub async fn ct_test_storage(
    cx: Arc<Backend>,
    arg: ArgUpsertStorage,
) -> BResult<StorageConnectionTestResult> {
    tokio_runtime().handle().spawn(async move {
        let arg = normalize_arg_upsert_storage(arg);
        let cx = cx.get_context();
        let backend = build_storage_backend_by_arg(cx, arg)?;
        let res = backend.list("/".to_string()).await;

        match res {
            Ok(_) => Ok(StorageConnectionTestResult::Success),
            Err(e) => {
                tracing::warn!("ct_test_storage, {e:?}");
                if e.is_unauthorized() {
                    Ok(StorageConnectionTestResult::Unauthorized)
                } else if e.is_timeout() {
                    Ok(StorageConnectionTestResult::Timeout)
                } else {
                    Ok(StorageConnectionTestResult::OtherError)
                }
            }
        }
    }).await.unwrap()
}

pub async fn ct_list_storage_entry_children(
    cx: Arc<Backend>,
    arg: StorageEntryLoc,
) -> BResult<ListStorageEntryChildrenResp> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        let backend = get_storage_backend(cx, arg.storage_id).await?;
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
                Ok(ListStorageEntryChildrenResp::Ok { data: entries })
            }
            Err(e) => {
                tracing::warn!("ct_list_storage_entry_children, {e:?}");
                if e.is_unauthorized() {
                    Ok(ListStorageEntryChildrenResp::AuthenticationFailed)
                } else if e.is_timeout() {
                    Ok(ListStorageEntryChildrenResp::Timeout)
                } else {
                    Ok(ListStorageEntryChildrenResp::Unknown)
                }
            }
        }
    }).await.unwrap()
}

pub fn ct_onedrive_oauth_url() -> String {
    onedrive_oauth_url()
}
