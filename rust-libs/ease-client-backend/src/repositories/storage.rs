use ease_client_shared::{ArgUpsertStorage, StorageId};
use ease_database::{params, DbConnectionRef};
use num_traits::ToPrimitive;

use crate::models::storage::StorageModel;

pub fn db_load_storage(
    conn: DbConnectionRef,
    storage_id: StorageId,
) -> anyhow::Result<Option<StorageModel>> {
    let model = conn
        .query::<StorageModel>("SELECT * FROM storage WHERE id = ?1", params![storage_id])?
        .pop();

    Ok(model)
}

pub fn db_load_storages(conn: DbConnectionRef) -> anyhow::Result<Vec<StorageModel>> {
    let model = conn.query::<StorageModel>("SELECT * FROM storage", params![])?;

    Ok(model)
}

pub fn db_upsert_storage(conn: DbConnectionRef, arg: ArgUpsertStorage) -> anyhow::Result<()> {
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

pub fn db_remove_storage(conn: DbConnectionRef, storage_id: StorageId) -> anyhow::Result<()> {
    conn.execute("DELETE FROM storage WHERE id = ?1", params![storage_id])?;
    Ok(())
}
