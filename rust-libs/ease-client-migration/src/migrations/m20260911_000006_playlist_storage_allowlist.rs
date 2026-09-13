//! Add `playlist.storage_allowlist` — the per-playlist import-source
//! restriction (NULL = all storages; JSON array of storage row ids =
//! only those storages may be imported from into that playlist).
//!
//! Real migration, deliberately NOT folded into the collapsed
//! `m20260715_000001_init`: databases already exist at the init schema
//! (dev installs + any shipped beta), so the column must be ALTERed in
//! for them. Fresh databases run init first and then this file — which
//! is exactly why the column must NOT also be added to the init CREATE
//! TABLE (fresh runs would fail with a duplicate column).
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
            .execute_unprepared("ALTER TABLE playlist ADD COLUMN storage_allowlist TEXT;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE playlist DROP COLUMN storage_allowlist;")
            .await?;
        Ok(())
    }
}
