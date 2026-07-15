use std::sync::Arc;

use ease_client_migration::entities::schema_version;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

use crate::error::BResult;

use super::core::DatabaseServer;

/// Re-export of the migration crate's schema version.
pub const SCHEMA_VERSION: u32 = ease_client_migration::SCHEMA_VERSION;

impl DatabaseServer {
    pub async fn delete_all(self: &Arc<Self>) -> BResult<()> {
        use sea_orm::ConnectionTrait;
        // Order matters because of FK relations; SQLite enforces PK/FK only
        // when explicitly enabled, so we drop in dependency order.
        let db = self.db();
        for stmt in [
            "DELETE FROM playlist_music",
            "DELETE FROM music",
            "DELETE FROM playlist",
            "DELETE FROM storage",
            "DELETE FROM preference",
            "DELETE FROM id_alloc",
            "DELETE FROM schema_version",
            "DELETE FROM blob",
        ] {
            db.execute_unprepared(stmt).await?;
        }
        Ok(())
    }

    pub async fn get_schema_version(self: &Arc<Self>) -> BResult<u32> {
        let db = self.db();
        let v = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
            .one(&db)
            .await?
            .map(|r| r.version)
            .unwrap_or(0);
        Ok(v)
    }

    pub async fn save_schema_version(self: &Arc<Self>, version: u32) -> BResult<()> {
        let db = self.db();
        let existing = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
            .one(&db)
            .await?;
        match existing {
            Some(row) => {
                let mut am: schema_version::ActiveModel = row.into();
                am.version = ActiveValue::Set(version);
                am.update(&db).await?;
            }
            None => {
                let am = schema_version::ActiveModel {
                    id: ActiveValue::Set(schema_version::Model::ROW_ID),
                    version: ActiveValue::Set(version),
                };
                am.insert(&db).await?;
            }
        }
        Ok(())
    }
}
