use std::sync::Arc;

use ease_client_shared::backends::storage::{ArgUpsertStorage, StorageId};
use num_traits::ToPrimitive;
use redb::{ReadTransaction, ReadableTable, ReadableTableMetadata};
use tracing::instrument;

use crate::{error::BResult, models::storage::StorageModel};

use super::{core::DatabaseServer, defs::TABLE_STORAGE};

impl DatabaseServer {
    fn load_storage_impl(
        self: &Arc<Self>,
        db: &ReadTransaction,
        id: StorageId,
    ) -> BResult<Option<StorageModel>> {
        let table = db.open_table(TABLE_STORAGE)?;
        let p = table.get(id)?.map(|v| v.value());
        Ok(p)
    }

    pub fn load_storages(self: &Arc<Self>) -> BResult<Vec<StorageModel>> {
        let db = self.db().begin_read()?;
        let table = db.open_table(TABLE_STORAGE)?;
        let len = table.len()? as usize;

        let mut ret: Vec<StorageModel> = Default::default();
        ret.reserve(len);

        let mut iter = table.iter()?;
        while let Some(v) = iter.next() {
            let v = v?.1.value();
            ret.push(v);
        }

        Ok(ret)
    }
}

#[instrument]
pub fn db_load_storage(
    conn: DbConnectionRef,
    storage_id: StorageId,
) -> BResult<Option<StorageModel>> {
    let model = conn
        .query::<StorageModel>("SELECT * FROM storage WHERE id = ?1", params![storage_id])?
        .pop();

    Ok(model)
}

#[instrument]
pub fn db_load_storages(conn: DbConnectionRef) -> BResult<Vec<StorageModel>> {
    let model = conn.query::<StorageModel>("SELECT * FROM storage", params![])?;

    Ok(model)
}

#[instrument]
pub fn db_upsert_storage(conn: DbConnectionRef, arg: ArgUpsertStorage) -> BResult<()> {
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

#[instrument]
pub fn db_remove_storage(conn: DbConnectionRef, storage_id: StorageId) -> BResult<()> {
    conn.execute("DELETE FROM storage WHERE id = ?1", params![storage_id])?;
    Ok(())
}
