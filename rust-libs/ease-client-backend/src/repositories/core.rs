use std::sync::{Arc, RwLock};

use ease_client_migration::DbConn;
use ease_client_migration::entities::{blob, id_alloc};
use ease_client_schema::DbKeyAlloc;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, EntityTrait};

use crate::error::BResult;

use super::blob::BlobManager;

#[derive(Default)]
pub struct DatabaseServer {
    db: RwLock<Option<(DbConn, Arc<BlobManager>)>>,
}

impl Drop for DatabaseServer {
    fn drop(&mut self) {
        self.db.write().unwrap().take();
        tracing::info!("drop DatabaseServer");
    }
}

impl DatabaseServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            db: Default::default(),
        })
    }

    /// Open the SQLite database file and run all schema migrations.
    /// Also seeds the blob-id counter row if missing.
    pub async fn init(&self, document_dir: String) -> BResult<()> {
        let db = ease_client_migration::migrate(&document_dir).await?;
        seed_blob_row(&db).await?;
        let blob_manager = BlobManager::open(document_dir + "blobs", db.clone());
        let mut w = self.db.write().unwrap();
        *w = Some((db, blob_manager));
        Ok(())
    }

    pub fn destroy(&self) {
        {
            let mut w = self.db.write().unwrap();
            *w = None;
        }
    }

    pub fn db(&self) -> DbConn {
        self.db.read().unwrap().clone().unwrap().0
    }

    pub fn blob(&self) -> Arc<BlobManager> {
        self.db.read().unwrap().clone().unwrap().1
    }

    /// Allocate a new ID. Used only for the legacy ID-allocation semantics
    /// (e.g. `BlobManager`). New entities should rely on SQLite autoincrement.
    #[allow(dead_code)]
    pub async fn alloc_id(&self, key: DbKeyAlloc) -> BResult<i64> {
        let kind = match key {
            DbKeyAlloc::Playlist => 0,
            DbKeyAlloc::Music => 1,
            DbKeyAlloc::Storage => 2,
        };

        let db = self.db();
        db.execute_unprepared("BEGIN IMMEDIATE").await?;
        let result = async {
            let existing = id_alloc::Entity::find_by_id(kind).one(&db).await?;
            let next_id = match existing {
                Some(row) => {
                    let cur = row.next_id;
                    let mut am: id_alloc::ActiveModel = row.into();
                    am.next_id = ActiveValue::Set(cur + 1);
                    am.update(&db).await?;
                    cur + 1
                }
                None => {
                    let am = id_alloc::ActiveModel {
                        kind: ActiveValue::Set(kind),
                        next_id: ActiveValue::Set(1),
                    };
                    am.insert(&db).await?;
                    1
                }
            };
            Ok::<_, sea_orm::DbErr>(next_id)
        }
        .await;
        db.execute_unprepared("COMMIT").await?;
        Ok(result?)
    }
}

async fn seed_blob_row(db: &DbConn) -> BResult<()> {
    let existing = blob::Entity::find_by_id(blob::Model::ROW_ID).one(db).await?;
    if existing.is_none() {
        let am = blob::ActiveModel {
            id: ActiveValue::Set(blob::Model::ROW_ID),
            next_id: ActiveValue::Set(1),
        };
        am.insert(db).await?;
    }
    Ok(())
}
