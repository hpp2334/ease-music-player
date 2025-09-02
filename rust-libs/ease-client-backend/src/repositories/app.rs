use std::sync::Arc;

use ease_client_schema::TABLE_SCHEMA_VERSION;

use crate::error::BResult;

use super::core::DatabaseServer;

impl DatabaseServer {
    pub fn delete_all(self: &Arc<Self>) -> BResult<()> {
        let db = self.db().begin_write()?;

        for t in db.list_tables()? {
            let _ = db.delete_table(t);
        }
        for t in db.list_multimap_tables()? {
            let _ = db.delete_multimap_table(t);
        }

        Ok(())
    }

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
}
