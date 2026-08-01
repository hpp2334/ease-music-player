use std::sync::Arc;
use ease_client_tokio::tokio_runtime;

use ease_client_schema::{StorageEntryLoc, StorageId};

use crate::{
    error::BResult,
    objects::{ListStorageEntryChildrenResp, Storage, StorageEntry},
    services::{get_storage_backend, list_storage, remove_storage},
    Backend,
};

pub async fn ct_list_storage(cx: Arc<Backend>) -> BResult<Vec<Storage>> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        let storages = list_storage(cx).await?;
        Ok(storages)
    }).await.unwrap()
}

pub async fn ct_remove_storage(cx: Arc<Backend>, id: StorageId) -> BResult<()> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        remove_storage(cx, id).await?;
        Ok(())
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
