use std::sync::Arc;

use ease_client_shared::backends::storage::BlobId;
use redb::WriteTransaction;

use crate::error::BResult;

use super::{
    core::DatabaseServer,
    defs::{TABLE_BLOB, TABLE_SCHEMA_VERSION},
};

impl DatabaseServer {
    pub fn get_schema_version(self: &Arc<Self>) -> BResult<u32> {
        let db = self.db().begin_read()?;
        let table = db.open_table(TABLE_SCHEMA_VERSION)?;
        let v = table.get(())?.map(|v| v.value()).unwrap_or_default();
        Ok(v)
    }

    pub fn save_schema_version(self: &Arc<Self>, version: u32) -> BResult<()> {
        let db = self.db().begin_write()?;
        {
            let mut table = db.open_table(TABLE_SCHEMA_VERSION)?;
            table.insert((), version)?;
        }
        db.commit()?;
        Ok(())
    }

    pub fn load_blob(self: &Arc<Self>, id: BlobId) -> BResult<Vec<u8>> {
        let db = self.db().begin_read()?;
        let table = db.open_table(TABLE_BLOB)?;
        let blob = table.get(id)?.unwrap().value();
        Ok(blob)
    }

    pub fn remove_blob_impl(self: &Arc<Self>, db: &WriteTransaction, id: BlobId) -> BResult<()> {
        let mut table = db.open_table(TABLE_BLOB)?;
        table.remove(id)?;
        Ok(())
    }
}
