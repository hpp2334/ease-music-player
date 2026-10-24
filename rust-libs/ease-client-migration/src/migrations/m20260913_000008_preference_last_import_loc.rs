//! Persist the last import folder (storage + path) on the single-row
//! `preference` table, as nullable JSON text — the Import page reopens
//! there on the next entry and silently falls back to `/` when the
//! folder can no longer be opened (Kotlin-side behavior).
//!
//! Real migration, deliberately NOT folded into the collapsed
//! `m20260715_000001_init`: databases already exist at the init schema
//! (dev installs + any shipped beta), so the column must be ALTERed in
//! for them. Fresh databases run init first and then this file — which
//! is exactly why it must NOT also be added to the init CREATE TABLE
//! (fresh runs would fail with duplicates).
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Raw SQL, same rationale as the statements folded into the init
        // migration: statement shape identical to what `sqlite3` would run.
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE preference ADD COLUMN last_import_loc text;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE preference DROP COLUMN last_import_loc;")
            .await?;
        Ok(())
    }
}
