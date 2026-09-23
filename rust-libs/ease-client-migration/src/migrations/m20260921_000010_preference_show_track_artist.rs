//! Whether track artist lines are shown in the UI (playlist rows, the
//! now-playing subtitle), on the single-row `preference` table. Ships
//! `DEFAULT 1` — enabled for both fresh and existing installs, so a
//! preference row that predates the column (or doesn't exist yet) still
//! shows artists.
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
            .execute_unprepared(
                "ALTER TABLE preference ADD COLUMN show_track_artist integer NOT NULL DEFAULT 1;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE preference DROP COLUMN show_track_artist;")
            .await?;
        Ok(())
    }
}
