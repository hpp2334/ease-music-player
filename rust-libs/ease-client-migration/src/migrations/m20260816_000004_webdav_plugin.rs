//! v6 -> v7: WebDAV stops being a native storage kind and becomes a JS plugin
//! provider (`com.ease.webdav`), exactly like OneDrive did in v6.
//!
//! Per WebDAV registry row (`type = 1`):
//!   - read its `webdav_storage` detail row (addr / alias / username /
//!     secret_id / is_anonymous);
//!   - re-file the password secret under the plugin's scope
//!     (`plugin:com.ease.webdav`, same secret id);
//!   - write a `plugin_kv` instance record the plugin backend reads:
//!     key `storage:webdav:<registry-id>` = JSON
//!     `{ alias, addr, username, isAnonymous, secretId }`;
//!   - rewrite the registry row to `type = 2` (Plugin),
//!     `plugin_id = 'com.ease.webdav'`,
//!     `plugin_storage_id = 'webdav:<registry-id>'`.
//!
//! Then drop the now-unused WebDAV structures: the `idx_storage_webdav`
//! index, the `webdav_storage` table, and the `storage.webdav_storage_id`
//! column.
//!
//! Fresh installs run v3 first (which creates the same intermediate shape),
//! so this migration needs no guarding either.

use std::time::{SystemTime, UNIX_EPOCH};

use ease_client_schema::entities::{plugin_kv_key, plugin_kv_single};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseBackend, Set, Statement};
use sea_orm_migration::prelude::*;

const WEBDAV_PLUGIN_ID: &str = "com.ease.webdav";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let sqlite = DatabaseBackend::Sqlite;

        // 1. Collect every native WebDAV registry row.
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                sqlite,
                "SELECT id, webdav_storage_id FROM storage WHERE type = 1",
                vec![],
            ))
            .await?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        for row in rows {
            let id = row.try_get::<i64>("", "id").unwrap_or(0);
            let wid = row
                .try_get::<i64>("", "webdav_storage_id")
                .unwrap_or(id);

            // 2. Read the detail row (joined defensively — a registry row
            //    whose detail went missing still becomes a plugin row, minus
            //    connection details).
            let detail = conn
                .query_one(Statement::from_sql_and_values(
                    sqlite,
                    "SELECT addr, alias, username, secret_id, is_anonymous FROM webdav_storage WHERE id = ?",
                    vec![wid.into()],
                ))
                .await?;
            let addr = detail
                .as_ref()
                .and_then(|r| r.try_get::<String>("", "addr").ok())
                .unwrap_or_default();
            let alias = detail
                .as_ref()
                .and_then(|r| r.try_get::<String>("", "alias").ok())
                .unwrap_or_default();
            let username = detail
                .as_ref()
                .and_then(|r| r.try_get::<String>("", "username").ok())
                .unwrap_or_default();
            let secret_id = detail
                .as_ref()
                .and_then(|r| r.try_get::<i64>("", "secret_id").ok());
            let is_anonymous = detail
                .as_ref()
                .and_then(|r| r.try_get::<i64>("", "is_anonymous").ok())
                .unwrap_or(0)
                != 0;

            // 3. Move the password secret under the plugin's scope (same id —
            //    the kv record below references it).
            if let Some(sid) = secret_id {
                conn.execute(Statement::from_sql_and_values(
                    sqlite,
                    "UPDATE secret SET scope = ? WHERE id = ?",
                    vec![format!("plugin:{}", WEBDAV_PLUGIN_ID).into(), sid.into()],
                ))
                .await?;
            }

            // 4. Write the plugin_kv instance record the backend reads.
            let instance = format!("webdav:{}", id);
            let instance_key = format!("storage:{}", instance);
            let key_am = plugin_kv_key::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                plugin_id: Set(WEBDAV_PLUGIN_ID.to_string()),
                key: Set(instance_key),
                kind: Set(0), // Single
                created_at: Set(now_ms),
            };
            let key_model = key_am.insert(conn).await?;
            let value = serde_json::json!({
                "alias": alias,
                "addr": addr,
                "username": username,
                "isAnonymous": is_anonymous,
                "secretId": secret_id,
            })
            .to_string();
            let single_am = plugin_kv_single::ActiveModel {
                key_id: Set(key_model.id),
                value: Set(value),
                updated_at: Set(now_ms),
            };
            single_am.insert(conn).await?;

            // 5. Rewrite the registry row to a Plugin row.
            conn.execute(Statement::from_sql_and_values(
                sqlite,
                "UPDATE storage SET type = 2, plugin_id = ?, plugin_storage_id = ?, \
                 webdav_storage_id = NULL WHERE id = ?",
                vec![
                    WEBDAV_PLUGIN_ID.into(),
                    instance.into(),
                    id.into(),
                ],
            ))
            .await?;
        }

        // 6. Drop the now-unused WebDAV structures.
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_storage_webdav")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS webdav_storage")
            .await?;
        conn.execute_unprepared("ALTER TABLE storage DROP COLUMN webdav_storage_id")
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only.
        Ok(())
    }
}
