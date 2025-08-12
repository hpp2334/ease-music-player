use std::sync::{Arc, RwLock};

use redb::{ReadableTable, WriteTransaction};

use crate::error::BResult;

use super::blob::BlobManager;
use ease_client_schema::{
    DbKeyAlloc, TABLE_ID_ALLOC, TABLE_MUSIC, TABLE_MUSIC_BY_LOC, TABLE_MUSIC_PLAYLIST,
    TABLE_PLAYLIST, TABLE_PLAYLIST_MUSIC, TABLE_PREFERENCE, TABLE_SCHEMA_VERSION, TABLE_STORAGE,
    TABLE_STORAGE_MUSIC,
};

#[derive(Default)]
pub struct DatabaseServer {
    _db: RwLock<Option<(Arc<redb::Database>, Arc<BlobManager>)>>,
}

impl Drop for DatabaseServer {
    fn drop(&mut self) {
        self._db.write().unwrap().take();
        tracing::info!("drop DatabaseServer");
    }
}

impl DatabaseServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            _db: Default::default(),
        })
    }

    pub fn init(&self, document_dir: String) {
        let p = document_dir.to_string() + "data.redb";
        {
            let mut w = self._db.write().unwrap();
            let db = redb::Database::builder()
                .set_cache_size(80 << 20)
                .create(&p)
                .expect("failed to init database");
            let blob_manager = BlobManager::open(document_dir + "blobs");
            *w = Some((Arc::new(db), blob_manager));
        }

        self.init_database().unwrap();
    }

    fn init_database(&self) -> BResult<()> {
        let db = self.db().begin_write()?;
        db.open_table(TABLE_ID_ALLOC)?;
        db.open_table(TABLE_PLAYLIST)?;
        db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
        db.open_multimap_table(TABLE_MUSIC_PLAYLIST)?;
        db.open_table(TABLE_MUSIC)?;
        db.open_table(TABLE_MUSIC_BY_LOC)?;
        db.open_table(TABLE_STORAGE)?;
        db.open_multimap_table(TABLE_STORAGE_MUSIC)?;
        db.open_table(TABLE_PREFERENCE)?;
        db.open_table(TABLE_SCHEMA_VERSION)?;
        db.commit()?;
        Ok(())
    }

    pub fn db(&self) -> Arc<redb::Database> {
        self._db.read().unwrap().clone().unwrap().0
    }

    pub fn blob(&self) -> Arc<BlobManager> {
        self._db.read().unwrap().clone().unwrap().1
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
