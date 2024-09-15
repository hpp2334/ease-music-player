use ease_client_shared::{ArgUpsertStorage, StorageId, StorageType};
use serde::{Deserialize, Serialize};

use crate::{
    ctx::Context,
    define_message,
    models::storage::StorageEntryLocModel,
    repositories::{
        core::get_conn,
        playlist::db_remove_musics_in_playlists_by_storage,
        storage::{db_load_storage, db_load_storages, db_remove_storage, db_upsert_storage},
    },
    services::storage::{build_storage, Storage},
};

use super::code::Code;

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageEntryLoc {
    pub path: String,
    pub storage_id: StorageId,
}

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
) -> anyhow::Result<Option<Vec<u8>>> {
    todo!()
}

define_message!(UpsertStorageMsg, Code::UpsertStorage, ArgUpsertStorage, ());
pub async fn ccu_upsert_storage(cx: Context, arg: ArgUpsertStorage) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    db_upsert_storage(conn.get_ref(), arg)?;
    Ok(())
}

define_message!(ListStorageMsg, Code::ListStorage, (), Vec<Storage>);
pub async fn cr_list_storage(cx: Context, _arg: ()) -> anyhow::Result<Vec<Storage>> {
    let conn = get_conn(&cx)?;
    let models = db_load_storages(conn.get_ref())?;
    let storages = models.into_iter().map(|m| build_storage(m)).collect();

    Ok(storages)
}

define_message!(GetStorageMsg, Code::GetStorage, StorageId, Option<Storage>);
pub async fn cr_get_storage(cx: Context, id: StorageId) -> anyhow::Result<Option<Storage>> {
    let conn = get_conn(&cx)?;
    let model = db_load_storage(conn.get_ref(), id)?;
    let storage = if let Some(model) = model {
        Some(build_storage(model))
    } else {
        None
    };
    Ok(storage)
}

define_message!(
    GetToRemoveStorageRefsMsg,
    Code::GetToRemoveStorageRefs,
    StorageId,
    Option<Storage>
);
pub async fn cr_get_to_remove_storage_refs(cx: Context, id: StorageId) -> anyhow::Result<()> {
    todo!()
}

define_message!(RemoveStorageMsg, Code::RemoveStorage, StorageId, ());
pub async fn cd_remove_storage(cx: Context, id: StorageId) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    db_remove_musics_in_playlists_by_storage(conn.get_ref(), id)?;
    db_remove_storage(conn.get_ref(), id)?;
    Ok(())
}

define_message!(TestStorageMsg, Code::TestStorage, ArgUpsertStorage, ());
pub async fn cr_test_storage(cx: Context, arg: ArgUpsertStorage) -> anyhow::Result<()> {
    todo!()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    typ: StorageType,
    name: String,
    path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ListStorageEntryChildrenResp {
    Ok(StorageEntry),
    AuthenticationFailed,
    Timeout,
}

define_message!(
    ListStorageEntryChildrenMsg,
    Code::ListStorageEntryChildren,
    StorageEntryLoc,
    ListStorageEntryChildrenResp
);
pub async fn cr_list_storage_entry_children(
    cx: Context,
    arg: StorageEntryLoc,
) -> anyhow::Result<ListStorageEntryChildrenResp> {
    todo!()
}
