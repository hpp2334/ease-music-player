use sea_orm_migration::prelude::*;

/// Adds the nullable `language` column to `preference` — the BCP-47 tag of
/// the in-app language override (NULL = follow the system locale). Raw SQL
/// like the plugin-kv migration: SQLite's `ADD COLUMN` has no
/// column-alter dance, and this keeps the statement shape identical to
/// what `sqlite3` would run.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE preference ADD COLUMN language TEXT NULL;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite (3.35+) supports DROP COLUMN; table-rebuild fallback is
        // unnecessary for a down path this suite never exercises on-device.
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE preference DROP COLUMN language;")
            .await?;
        Ok(())
    }
}
