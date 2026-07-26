use sea_orm_migration::prelude::*;

/// Adds the three plugin-KV tables: `plugin_kv_key` (key registry),
/// `plugin_kv_single` (one value per key, overwrite), and
/// `plugin_kv_multi` (many values per key, append-only).
///
/// Each (plugin_id, key) pair is registered exactly once in
/// `plugin_kv_key` and tagged Single or Multi. Single and Multi rows
/// reference `plugin_kv_key.id` so the (plugin_id, key) string pair is
/// stored once, not on every value row.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "
                CREATE TABLE IF NOT EXISTS plugin_kv_key (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    plugin_id   TEXT    NOT NULL,
                    key         TEXT    NOT NULL,
                    kind        INTEGER NOT NULL,
                    created_at  INTEGER NOT NULL,
                    UNIQUE (plugin_id, key)
                );
                CREATE INDEX IF NOT EXISTS idx_plugin_kv_key_plugin
                    ON plugin_kv_key (plugin_id);

                CREATE TABLE IF NOT EXISTS plugin_kv_single (
                    key_id      INTEGER PRIMARY KEY,
                    value       TEXT    NOT NULL,
                    updated_at  INTEGER NOT NULL,
                    FOREIGN KEY (key_id) REFERENCES plugin_kv_key(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS plugin_kv_multi (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    key_id      INTEGER NOT NULL,
                    value       TEXT    NOT NULL,
                    created_at  INTEGER NOT NULL,
                    FOREIGN KEY (key_id) REFERENCES plugin_kv_key(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_plugin_kv_multi_key_id
                    ON plugin_kv_multi (key_id, id);
                ",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "
                DROP TABLE IF EXISTS plugin_kv_multi;
                DROP TABLE IF EXISTS plugin_kv_single;
                DROP TABLE IF EXISTS plugin_kv_key;
                ",
            )
            .await?;
        Ok(())
    }
}
