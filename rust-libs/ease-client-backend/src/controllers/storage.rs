use ease_client_shared::backends::{
    message::IMessage,
    storage::{
        ArgUpsertStorage, ListStorageEntryChildrenResp, Storage, StorageConnectionTestResult,
        StorageEntry, StorageEntryLoc, StorageId, TestStorageMsg,
    },
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::storage::StorageEntryLocModel,
    repositories::{
        core::get_conn,
        playlist::db_remove_musics_in_playlists_by_storage,
        storage::{db_load_storage, db_load_storages, db_remove_storage, db_upsert_storage},
    },
    services::storage::{build_storage, get_storage_backend, get_storage_backend_by_arg},
};

pub(crate) fn to_opt_storage_entry(
    path: Option<String>,
    id: Option<StorageId>,
) -> Option<StorageEntryLoc> {
    if path.is_some() && id.is_some() {
        Some(StorageEntryLoc {
            path: path.unwrap(),
            storage_id: id.unwrap(),
        })
    } else {
        None
    }
}

pub(crate) fn from_opt_storage_entry(loc: Option<StorageEntryLoc>) -> StorageEntryLocModel {
    if let Some(loc) = loc {
        (Some(loc.path), Some(loc.storage_id))
    } else {
        (None, None)
    }
}

pub(crate) async fn load_storage_entry_data(
    cx: &BackendContext,
    loc: &StorageEntryLoc,
) -> BResult<Option<Vec<u8>>> {
    let backend = get_storage_backend(cx, loc.storage_id)?;
    if let Some(backend) = backend {
        match backend.get(&loc.path).await {
            Ok(data) => {
                let data = data.bytes().await?;
                let data = data.to_vec();
                Ok(Some(data))
            }
            Err(e) => Ok(None),
        }
    } else {
        Ok(None)
    }
}

pub async fn ccu_upsert_storage(cx: BackendContext, arg: ArgUpsertStorage) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_upsert_storage(conn.get_ref(), arg)?;
    Ok(())
}

pub async fn cr_list_storage(cx: BackendContext, _arg: ()) -> BResult<Vec<Storage>> {
    let conn = get_conn(&cx)?;
    let models = db_load_storages(conn.get_ref())?;
    let storages = models.into_iter().map(|m| build_storage(m)).collect();

    Ok(storages)
}

pub async fn cr_get_storage(cx: BackendContext, id: StorageId) -> BResult<Option<Storage>> {
    let conn = get_conn(&cx)?;
    let model = db_load_storage(conn.get_ref(), id)?;
    let storage = if let Some(model) = model {
        Some(build_storage(model))
    } else {
        None
    };
    Ok(storage)
}

pub async fn cd_remove_storage(cx: BackendContext, id: StorageId) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_remove_musics_in_playlists_by_storage(conn.get_ref(), id)?;
    db_remove_storage(conn.get_ref(), id)?;
    Ok(())
}

pub async fn cr_test_storage(
    cx: BackendContext,
    arg: <TestStorageMsg as IMessage>::Argument,
) -> BResult<<TestStorageMsg as IMessage>::Return> {
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
    cx: BackendContext,
    arg: StorageEntryLoc,
) -> BResult<ListStorageEntryChildrenResp> {
    let backend = get_storage_backend(&cx, arg.storage_id)?;
    if backend.is_none() {
        return Ok(ListStorageEntryChildrenResp::Unknown);
    }
    let backend = backend.unwrap();

    let res = backend.list(&arg.path).await;

    match res {
        Ok(entries) => {
            let entries = entries
                .into_iter()
                .map(|entry| StorageEntry {
                    name: entry.name,
                    path: entry.path,
                    size: entry.size,
                    is_dir: entry.is_dir,
                })
                .collect();
            Ok(ListStorageEntryChildrenResp::Ok(entries))
        }
        Err(e) => {
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
