//! Introduce playlist groups — a named, ordered container of playlists.
//!
//! - New `playlist_group` table (id / title / created_time / order /
//!   expanded). `expanded` persists the per-group UI expand/collapse
//!   state (written on every user toggle; default 1 = expanded).
//! - `playlist.group_id` column referencing it. Added as NULLable on
//!   purpose: pre-group databases have existing playlists with no group,
//!   and the Default-group creation + orphan sweep is done at runtime
//!   (`playlistGroup.ensureDefault`) rather than in SQL — the sweep
//!   needs the localized group title that only the Kotlin caller knows.
//!
//! Real migration, deliberately NOT folded into the collapsed
//! `m20260715_000001_init`: databases already exist at the init schema
//! (dev installs + any shipped beta), so the table + column must be
//! CREATEd / ALTERed in for them. Fresh databases run init first and
//! then this file — which is exactly why they must NOT also be added to
//! the init CREATE TABLE (fresh runs would fail with duplicates).
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
                "CREATE TABLE IF NOT EXISTS \"playlist_group\" (
                    \"id\" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
                    \"title\" text NOT NULL DEFAULT '',
                    \"created_time\" bigint NOT NULL DEFAULT 0,
                    \"order\" text NOT NULL DEFAULT '[]',
                    \"expanded\" integer NOT NULL DEFAULT 1
                );",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE playlist ADD COLUMN group_id bigint;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE playlist DROP COLUMN group_id;")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS \"playlist_group\";")
            .await?;
        Ok(())
    }
}
