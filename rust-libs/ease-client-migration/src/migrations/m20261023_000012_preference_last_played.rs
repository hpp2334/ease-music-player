//! Persist the last-played track (music + playlist id pair) on the
//! single-row `preference` table, as nullable JSON text — the player
//! rehydrates from it on startup so a process kill while backgrounded
//! restores the player page / mini-player as paused instead of empty
//! (Kotlin-side behavior; the ids are re-validated against the live
//! playlist before use).
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
            .execute_unprepared("ALTER TABLE preference ADD COLUMN last_played text;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE preference DROP COLUMN last_played;")
            .await?;
        Ok(())
    }
}
