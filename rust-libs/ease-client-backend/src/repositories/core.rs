use std::sync::{Arc, RwLock};

use redb::{ReadableTable, WriteTransaction};

use crate::{error::BResult, models::key::DbKeyAlloc};

use super::defs::TABLE_ID_ALLOC;

#[derive(Default)]
pub struct DatabaseServer {
    _db: RwLock<Option<Arc<redb::Database>>>,
}

impl DatabaseServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            _db: Default::default(),
        })
    }

    pub fn init(&self, document_dir: String) {
        let p = document_dir + "data.redb";
        {
            let mut w = self._db.write().unwrap();
            let db = redb::Database::builder()
                .set_cache_size(80 << 20)
                .create(&p)
                .expect("failed to init database");
            *w = Some(Arc::new(db));
        }
    }

    pub fn db(&self) -> Arc<redb::Database> {
        self._db.read().unwrap().clone().unwrap()
    }

    pub fn alloc_id(&self, db: &WriteTransaction, key: DbKeyAlloc) -> BResult<i64> {
        let next_id = {
            let mut table = db.open_table(TABLE_ID_ALLOC)?;
            let id = table.get(key)?.map(|v| v.value()).unwrap_or_default();
            let next_id = id + 1;
            table.insert(key, next_id)?;
            next_id
        };
        Ok(next_id)
    }
}
