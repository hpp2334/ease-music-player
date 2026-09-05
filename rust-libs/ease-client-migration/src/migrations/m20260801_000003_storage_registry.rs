//! v5 -> v6: turn the flat `storage` table (Local/Webdav/OneDrive rows with
//! inline `addr`/`username`/`password`/`typ`) into a kind-agnostic **registry**
//! (`storage`) backed by two detail tables:
//!   - `webdav_storage` — WebDAV connection details.
//!   - `secret`         — opaque secret values scoped by owner
//!                        (`"internal"` for host/WebDAV, `"plugin:<id>"` for
//!                        plugin-owned secrets).
//!
//! Per-row data moves:
//!   - WebDAV row  -> `webdav_storage` row (same id) + `secret` (internal) +
//!                    registry row (type=Webdav, webdav_storage_id=id).
//!   - OneDrive row -> `secret` (scope=`plugin:com.ease.onedrive`) + a
//!                     `plugin_kv` instance record holding `{alias, secretId}`
//!                     + registry row (type=Plugin, plugin_id, plugin_storage_id).
//!   - Local row    -> registry row (type=Local). Music/playlist `*_storage_id`
//!                     sentinels (-1, the synthetic Local id) are remapped to the
//!                     Local registry row id.
//!
//! The same migration transforms fresh installs (init created the old shape) —
//! no guarding needed.

use std::time::{SystemTime, UNIX_EPOCH};

use ease_client_schema::entities::{plugin_kv_key, plugin_kv_single, secret, webdav_storage};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

const ONEDRIVE_PLUGIN_ID: &str = "com.ease.onedrive";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let sqlite = DatabaseBackend::Sqlite;

        // 1. Create the two new detail tables.
        conn.execute_unprepared(
            r#"CREATE TABLE IF NOT EXISTS webdav_storage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                addr TEXT NOT NULL DEFAULT '',
                alias TEXT NOT NULL DEFAULT '',
                username TEXT NOT NULL DEFAULT '',
                secret_id INTEGER,
                is_anonymous INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .await?;
        conn.execute_unprepared(
            r#"CREATE TABLE IF NOT EXISTS secret (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scope TEXT NOT NULL DEFAULT 'internal',
                secret TEXT NOT NULL DEFAULT ''
            )"#,
        )
        .await?;

        // 2. Resolve / create the Local registry row id.
        let local_id: i64 = match conn
            .query_one(Statement::from_sql_and_values(
                sqlite,
                "SELECT id FROM storage WHERE typ = 0 ORDER BY id ASC LIMIT 1",
                vec![],
            ))
            .await?
        {
            Some(row) => row.try_get::<i64>("", "id").unwrap(),
            None => {
                conn.execute_unprepared(
                    "INSERT INTO storage (addr, alias, username, password, is_anonymous, typ) \
                     VALUES ('', 'Local', '', '', 0, 0)",
                )
                .await?;
                conn.query_one(Statement::from_sql_and_values(
                    sqlite,
                    "SELECT id FROM storage WHERE typ = 0 ORDER BY id DESC LIMIT 1",
                    vec![],
                ))
                .await?
                .unwrap()
                .try_get::<i64>("", "id")
                .unwrap()
            }
        };

        // 3. Read every legacy storage row (the entity no longer matches this shape).
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                sqlite,
                "SELECT id, addr, alias, username, password, is_anonymous, typ FROM storage",
                vec![],
            ))
            .await?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        for row in rows {
            let id = row.try_get::<i64>("", "id").unwrap_or(0);
            let addr = row.try_get::<String>("", "addr").unwrap_or_default();
            let alias = row.try_get::<String>("", "alias").unwrap_or_default();
            let username = row.try_get::<String>("", "username").unwrap_or_default();
            let password = row.try_get::<String>("", "password").unwrap_or_default();
            let is_anonymous = row.try_get::<i64>("", "is_anonymous").unwrap_or(0) != 0;
            let typ = row.try_get::<i64>("", "typ").unwrap_or(1);

            match typ {
                // WebDAV: split into webdav_storage + secret (internal).
                1 => {
                    let secret_id = if password.is_empty() {
                        None
                    } else {
                        let am = secret::ActiveModel {
                            id: sea_orm::ActiveValue::NotSet,
                            scope: Set("internal".to_string()),
                            secret: Set(password),
                        };
                        let m = am.insert(conn).await?;
                        Some(m.id)
                    };
                    let am = webdav_storage::ActiveModel {
                        id: Set(id),
                        addr: Set(addr),
                        alias: Set(alias),
                        username: Set(username),
                        secret_id: Set(secret_id),
                        is_anonymous: Set(if is_anonymous { 1 } else { 0 }),
                    };
                    am.insert(conn).await?;
                }
                // OneDrive: secret (plugin scope) + plugin_kv instance record.
                2 => {
                    let secret_id = if password.is_empty() {
                        None
                    } else {
                        let am = secret::ActiveModel {
                            id: sea_orm::ActiveValue::NotSet,
                            scope: Set(format!("plugin:{}", ONEDRIVE_PLUGIN_ID)),
                            secret: Set(password),
                        };
                        let m = am.insert(conn).await?;
                        Some(m.id)
                    };
                    let instance_key = format!("storage:onedrive:{}", id);
                    let key_am = plugin_kv_key::ActiveModel {
                        id: sea_orm::ActiveValue::NotSet,
                        plugin_id: Set(ONEDRIVE_PLUGIN_ID.to_string()),
                        key: Set(instance_key),
                        kind: Set(0), // Single
                        created_at: Set(now_ms),
                    };
                    let key_model = key_am.insert(conn).await?;
                    let value = serde_json::json!({
                        "alias": alias,
                        "secretId": secret_id,
                    })
                    .to_string();
                    let single_am = plugin_kv_single::ActiveModel {
                        key_id: Set(key_model.id),
                        value: Set(value),
                        updated_at: Set(now_ms),
                    };
                    single_am.insert(conn).await?;
                }
                // Local: nothing to split; the registry row keeps type=Local.
                _ => {}
            }
        }

        // 4. Add the new registry columns.
        for stmt in [
            "ALTER TABLE storage ADD COLUMN new_type INTEGER",
            "ALTER TABLE storage ADD COLUMN webdav_storage_id INTEGER",
            "ALTER TABLE storage ADD COLUMN plugin_id TEXT",
            "ALTER TABLE storage ADD COLUMN plugin_storage_id TEXT",
        ] {
            conn.execute_unprepared(stmt).await?;
        }

        // 5. Backfill the registry columns from the legacy `typ`.
        conn.execute_unprepared("UPDATE storage SET new_type = 0 WHERE typ = 0")
            .await?;
        conn.execute_unprepared(
            "UPDATE storage SET new_type = 1, webdav_storage_id = id WHERE typ = 1",
        )
        .await?;
        conn.execute_unprepared(&format!(
            "UPDATE storage SET new_type = 2, plugin_id = '{pid}', \
             plugin_storage_id = 'onedrive:' || CAST(id AS TEXT) WHERE typ = 2",
            pid = ONEDRIVE_PLUGIN_ID
        ))
        .await?;

        // 6. Remap the synthetic Local sentinel (-1) in music / playlist to the
        //    Local registry row id.
        let lid = local_id.to_string();
        conn.execute_unprepared(&format!(
            "UPDATE music SET loc_storage_id = {lid} WHERE loc_storage_id = -1"
        ))
        .await?;
        conn.execute_unprepared(&format!(
            "UPDATE music SET lyric_storage_id = {lid} WHERE lyric_storage_id = -1"
        ))
        .await?;
        conn.execute_unprepared(&format!(
            "UPDATE playlist SET picture_storage_id = {lid} WHERE picture_storage_id = -1"
        ))
        .await?;

        // 7. Drop the legacy columns and rename `new_type` -> `type`.
        for col in ["addr", "alias", "username", "password", "is_anonymous", "typ"] {
            conn.execute_unprepared(&format!("ALTER TABLE storage DROP COLUMN {col}"))
                .await?;
        }
        conn.execute_unprepared("ALTER TABLE storage RENAME COLUMN new_type TO type")
            .await?;

        // 8. Per-kind uniqueness for find-or-create (`obtain`) safety.
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_storage_webdav ON storage (webdav_storage_id)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_storage_plugin \
             ON storage (plugin_id, plugin_storage_id)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only.
        Ok(())
    }
}
