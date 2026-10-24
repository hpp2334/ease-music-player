//! Track artist on the `music` table, captured from container tags
//! (`Artist`, falling back to `AlbumArtist`) by the `player.loadMusic`
//! metadata writeback. Empty string = never probed or no usable tag.
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
                "ALTER TABLE music ADD COLUMN artist text NOT NULL DEFAULT '';",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE music DROP COLUMN artist;")
            .await?;
        Ok(())
    }
}
