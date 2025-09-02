use std::sync::Arc;

use ease_client_schema::{PreferenceModel, TABLE_PREFERENCE};

use crate::error::BResult;

use super::core::DatabaseServer;

impl DatabaseServer {
    pub fn load_preference(self: &Arc<Self>) -> BResult<PreferenceModel> {
        let db = self.db().begin_read()?;
        let table = db.open_table(TABLE_PREFERENCE)?;
        let v = table.get(())?.map(|v| v.value()).unwrap_or_default();
        Ok(v)
    }

    pub fn save_preference(self: &Arc<Self>, model: PreferenceModel) -> BResult<()> {
        let db = self.db().begin_write()?;
        {
            let mut table = db.open_table(TABLE_PREFERENCE)?;
            table.insert((), model)?;
        }
        db.commit()?;
        Ok(())
    }
}
