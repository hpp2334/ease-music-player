use std::{path::PathBuf, sync::Arc};

use ease_client_migration::DbConn;
use ease_client_schema::entities::blob;
use ease_client_schema::BlobId;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, EntityTrait};

use crate::error::BResult;

pub struct BlobManager {
    dir: String,
    db: DbConn,
}

fn blobs_path(dir: &str) -> PathBuf {
    std::path::Path::new(dir).join("blobs")
}

fn blob_path(dir: &str, index: BlobId) -> PathBuf {
    std::path::Path::new(dir)
        .join("blobs")
        .join(index.as_ref().to_string())
}

impl BlobManager {
    /// Opens the blob directory. The blob-id counter lives in the main
    /// application database (`db`), so this does not need its own DB file.
    pub fn open(dir: String, db: DbConn) -> Arc<Self> {
        std::fs::create_dir_all(&dir).expect("Failed to create directory");
        std::fs::create_dir_all(blobs_path(&dir)).expect("Failed to create directory");
        Arc::new(Self { dir, db })
    }

    pub fn read(&self, id: BlobId) -> BResult<Vec<u8>> {
        let path = blob_path(self.dir.as_str(), id);
        let ret = std::fs::read(path)?;
        Ok(ret)
    }

    pub fn remove(&self, id: BlobId) -> BResult<()> {
        let path = blob_path(self.dir.as_str(), id);
        std::fs::remove_file(path)?;
        Ok(())
    }

    pub async fn write(&self, buf: Vec<u8>) -> BResult<BlobId> {
        let id = self.allocate().await?;
        let path = blob_path(self.dir.as_str(), id);
        std::fs::write(path, buf)?;
        Ok(id)
    }

    async fn allocate(&self) -> BResult<BlobId> {
        use sea_orm::TransactionTrait;

        // Serialize allocation against itself (`SQLITE_BUSY_SNAPSHOT`
        // upgrade races — see `DatabaseServer::alloc_id`).
        static ALLOC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = ALLOC_LOCK.lock().await;

        // One transaction, ONE pooled connection (see
        // `DatabaseServer::alloc_id` for why the historical
        // `execute_unprepared("BEGIN …/COMMIT")` pattern was broken).
        let txn = self.db.begin().await?;
        let row = blob::Entity::find_by_id(blob::Model::ROW_ID)
            .one(&txn)
            .await?
            .expect("blob row is seeded by ease_client_migration::migrate");
        let cur = row.next_id;
        let mut am: blob::ActiveModel = row.into();
        am.next_id = ActiveValue::Set(cur + 1);
        am.update(&txn).await?;
        txn.commit().await?;
        Ok(BlobId::wrap(cur))
    }
}
