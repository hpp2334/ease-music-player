use ease_client_shared::backends::storage::{
    ArgUpsertStorage, ListStorageEntryChildrenResp, Storage, StorageConnectionTestResult,
    StorageEntryLoc, StorageId,
};

use crate::{
    ctx::Context,
    error::BResult,
    models::storage::StorageEntryLocModel,
    repositories::{
        core::get_conn,
        playlist::db_remove_musics_in_playlists_by_storage,
        storage::{db_load_storage, db_load_storages, db_remove_storage, db_upsert_storage},
    },
    services::storage::build_storage,
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
    cx: &Context,
    loc: &StorageEntryLoc,
) -> BResult<Option<Vec<u8>>> {
    todo!()
}

pub async fn ccu_upsert_storage(cx: Context, arg: ArgUpsertStorage) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_upsert_storage(conn.get_ref(), arg)?;
    Ok(())
}

pub async fn cr_list_storage(cx: Context, _arg: ()) -> BResult<Vec<Storage>> {
    let conn = get_conn(&cx)?;
    let models = db_load_storages(conn.get_ref())?;
    let storages = models.into_iter().map(|m| build_storage(m)).collect();

    Ok(storages)
}

pub async fn cr_get_storage(cx: Context, id: StorageId) -> BResult<Option<Storage>> {
    let conn = get_conn(&cx)?;
    let model = db_load_storage(conn.get_ref(), id)?;
    let storage = if let Some(model) = model {
        Some(build_storage(model))
    } else {
        None
    };
    Ok(storage)
}

pub async fn cr_get_to_remove_storage_refs(cx: Context, id: StorageId) -> BResult<()> {
    todo!()
}

pub async fn cd_remove_storage(cx: Context, id: StorageId) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_remove_musics_in_playlists_by_storage(conn.get_ref(), id)?;
    db_remove_storage(conn.get_ref(), id)?;
    Ok(())
}

pub async fn cr_test_storage(
    cx: Context,
    arg: ArgUpsertStorage,
) -> BResult<StorageConnectionTestResult> {
    todo!()
}

pub async fn cr_list_storage_entry_children(
    cx: Context,
    arg: StorageEntryLoc,
) -> BResult<ListStorageEntryChildrenResp> {
    todo!()
}
