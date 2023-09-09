use std::collections::HashMap;

use ease_database::{params, DbConnectionRef};
use misty_vm::client::{AsReadonlyMistyClientHandle, MistyClientHandle};
use num_traits::ToPrimitive;

use crate::modules::{
    app::service::get_db_conn_v2,
    error::{EaseResult, EASE_RESULT_NIL},
    storage::repository::StorageModel,
};

use super::super::{ArgUpsertStorage, Storage, StorageId, StorageInfo, StorageType};

pub fn db_load_storage<'a>(
    app: impl AsReadonlyMistyClientHandle<'a>,
    storage_id: StorageId,
) -> EaseResult<Storage> {
    let conn = get_db_conn_v2(app)?;

    let model = conn
        .query::<StorageModel>("SELECT * FROM storage WHERE id = ?1", params![storage_id])?
        .pop()
        .unwrap();

    Ok(Storage { model })
}

fn db_upsert_storage_impl(
    conn: DbConnectionRef,
    arg: ArgUpsertStorage,
) -> ease_database::Result<()> {
    let typ: i32 = arg.typ.to_i32().unwrap();

    if arg.id.is_none() {
        let typ: i32 = arg.typ.to_i32().unwrap();

        conn.execute(
            r#"
            INSERT INTO storage (addr, alias, username, password, is_anonymous, typ)
            values (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                arg.addr,
                arg.alias,
                arg.username,
                arg.password,
                arg.is_anonymous,
                typ
            ],
        )?;
    } else {
        let id = arg.id.unwrap();

        conn.execute(
            r#"UPDATE storage SET
            addr = ?1, alias = ?2, username = ?3, password = ?4,
            is_anonymous = ?5, typ = ?6 WHERE id = ?7"#,
            params![
                arg.addr,
                arg.alias,
                arg.username,
                arg.password,
                arg.is_anonymous,
                typ,
                id
            ],
        )?;
    }

    return Ok(());
}

pub fn db_upsert_storage(app: MistyClientHandle, arg: ArgUpsertStorage) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;
    db_upsert_storage_impl(conn.get_ref(), arg)?;
    return EASE_RESULT_NIL;
}

pub fn db_init_local_storage_db_if_not_exist(app: MistyClientHandle) -> EaseResult<()> {
    let mut conn = get_db_conn_v2(app)?;

    conn.transaction(|conn| {
        let local_typ: i32 = StorageType::Local.to_i32().unwrap();

        let existed_local_storage =
            conn.query::<i32>("SELECT id FROM storage WHERE typ = ?1", params![local_typ])?;

        if !existed_local_storage.is_empty() {
            return Ok(());
        }

        db_upsert_storage_impl(
            conn,
            ArgUpsertStorage {
                id: None,
                addr: Default::default(),
                alias: Some("Local".to_string()),
                username: Default::default(),
                password: Default::default(),
                is_anonymous: true,
                typ: StorageType::Local,
            },
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn db_remove_storage(app: MistyClientHandle, storage_id: StorageId) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;

    conn.execute("DELETE FROM storage WHERE id = ?1", params![storage_id])?;
    Ok(())
}

pub fn db_load_storage_infos(
    app: MistyClientHandle,
) -> EaseResult<HashMap<StorageId, StorageInfo>> {
    let conn = get_db_conn_v2(app)?;

    let list = conn.query::<StorageModel>("SELECT * FROM storage", [])?;

    let list: Vec<StorageInfo> = list
        .into_iter()
        .map(|item| Storage { model: item })
        .map(|item| StorageInfo {
            id: item.id(),
            name: item.display_name(),
            addr: item.addr().to_string(),
            typ: item.typ(),
        })
        .collect();

    let mut map: HashMap<StorageId, StorageInfo> = Default::default();
    for v in list.into_iter() {
        map.insert(v.id, v);
    }
    Ok(map)
}
