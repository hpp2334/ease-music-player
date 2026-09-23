//! Tombstone of the former storage-registry migration, folded into
//! `m20260715_000001_init` when the v4-internal chain was collapsed
//! (v0.4 never shipped; only the v3 (redb) -> v4 import is considered).
//!
//! The module must keep its exact name: databases that already applied
//! the old chain record this version in `seaql_migrations`, and
//! sea-orm-migration hard-errors when an applied version has no file in
//! the Migrator's list. `up`/`down` are deliberate no-ops — the init
//! migration already creates the final schema.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
