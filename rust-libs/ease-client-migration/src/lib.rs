use std::path::PathBuf;
use std::sync::Arc;

use ease_client_schema::v3;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait};
use sea_orm_migration::MigratorTrait;

pub mod converter;
pub mod entities;
pub mod legacy;
pub mod migrations;

pub use sea_orm::DatabaseConnection as DbConn;

use crate::entities::{blob, id_alloc, music, playlist, playlist_music, preference, schema_version, storage};
use crate::legacy::v3::TABLE_BLOB as V3_TABLE_BLOB;
use crate::legacy::v3::TABLE_ID_ALLOC as V3_TABLE_ID_ALLOC;
use crate::legacy::v3::TABLE_MUSIC as V3_TABLE_MUSIC;
use crate::legacy::v3::TABLE_MUSIC_PLAYLIST as V3_TABLE_MUSIC_PLAYLIST;
use crate::legacy::v3::TABLE_PLAYLIST as V3_TABLE_PLAYLIST;
use crate::legacy::v3::TABLE_PREFERENCE as V3_TABLE_PREFERENCE;
use crate::legacy::v3::TABLE_STORAGE as V3_TABLE_STORAGE;
use crate::legacy::v3::TABLE_SCHEMA_VERSION as V3_TABLE_SCHEMA_VERSION;
use crate::legacy::{upgrade_v1_to_v2, upgrade_v2_to_v3};
use redb::{ReadableMultimapTable, ReadableTable};

/// The schema version produced by this crate.
pub const SCHEMA_VERSION: u32 = 4;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        vec![Box::new(migrations::InitMigration)]
    }
}

fn data_db_path(document_dir: &str) -> PathBuf {
    PathBuf::from(document_dir).join("data.db")
}

fn legacy_redb_path(document_dir: &str) -> PathBuf {
    PathBuf::from(document_dir).join("data.redb")
}

/// Open or create the SQLite database at `<document_dir>/data.db`.
pub async fn open_database(document_dir: &str) -> anyhow::Result<DbConn> {
    let path = data_db_path(document_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let db = sea_orm::Database::connect(&url).await?;
    Ok(db)
}

/// Apply the latest sea-orm-migration schema. Idempotent.
pub async fn init_schema(db: &DbConn) -> anyhow::Result<()> {
    Migrator::up(db, None).await?;
    Ok(())
}

/// Read the legacy `data.redb` next to the SQLite file, run any in-redb
/// upgrades (v1->v2, v2->v3), then stream every v3 row into SQLite and
/// bump the SQLite schema_version to 4. On success the redb file is deleted.
pub async fn import_from_redb(
    src_path: &std::path::Path,
    dst: &DbConn,
) -> anyhow::Result<()> {
    let redb_db = Arc::new(redb::Database::open(src_path)?);

    let src_version = read_redb_schema_version(&redb_db)?;

    if src_version < 2 {
        upgrade_v1_to_v2(&redb_db)?;
    }
    if src_version < 3 {
        upgrade_v2_to_v3(&redb_db)?;
    }

    let txn = dst.begin().await?;

    let playlist_rows = read_all_playlists(&redb_db)?;
    for m in playlist_rows {
        converter::playlist_from(m).insert(&txn).await?;
    }

    let music_rows = read_all_music(&redb_db)?;
    for m in music_rows {
        converter::music_from(m).insert(&txn).await?;
    }

    let storage_rows = read_all_storage(&redb_db)?;
    for m in storage_rows {
        converter::storage_from(m).insert(&txn).await?;
    }

    let playlist_music_rows = read_all_playlist_music(&redb_db)?;
    for (pid, mid) in playlist_music_rows {
        converter::playlist_music_from(pid, mid).insert(&txn).await?;
    }

    if let Some(pref) = read_preference(&redb_db)? {
        converter::preference_from(pref).insert(&txn).await?;
    }

    for (kind, next_id) in read_id_alloc(&redb_db)? {
        converter::id_alloc_from(kind, next_id).insert(&txn).await?;
    }

    if let Some(next_blob_id) = read_blob_next_id(&redb_db)? {
        converter::blob_alloc_from(next_blob_id).insert(&txn).await?;
    }

    upsert_schema_version(&txn, SCHEMA_VERSION).await?;

    txn.commit().await?;

    tracing::info!("v3 -> v4: finished import from redb");
    Ok(())
}

/// Run on every startup. Idempotent.
///
/// 1. Opens/creates `<document_dir>/data.db`.
/// 2. Runs sea-orm-migration `Migrator::up` to the latest schema.
/// 3. If a legacy `data.redb` exists in the same directory AND the SQLite
///    schema_version is below [`SCHEMA_VERSION`], imports all rows from redb
///    into SQLite and deletes the redb file.
pub async fn migrate(document_dir: &str) -> anyhow::Result<DbConn> {
    let db = open_database(document_dir).await?;
    init_schema(&db).await?;

    let current_version = read_schema_version(&db).await?;
    let redb_path = legacy_redb_path(document_dir);

    if current_version < SCHEMA_VERSION && redb_path.exists() {
        tracing::info!(
            "found legacy redb at {}, schema_version={}; running import",
            redb_path.display(),
            current_version
        );
        import_from_redb(&redb_path, &db).await?;

        if let Err(e) = std::fs::remove_file(&redb_path) {
            tracing::warn!("failed to remove legacy {}: {}", redb_path.display(), e);
        } else {
            tracing::info!("removed legacy {}", redb_path.display());
        }
    } else if current_version == 0 {
        // Fresh install with no legacy redb: just stamp the version.
        let txn = db.begin().await?;
        upsert_schema_version(&txn, SCHEMA_VERSION).await?;
        txn.commit().await?;
    }

    Ok(db)
}

async fn read_schema_version(db: &DbConn) -> anyhow::Result<u32> {
    let row = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(db)
        .await?;
    Ok(row.map(|r| r.version).unwrap_or(0))
}

async fn upsert_schema_version<C: ConnectionTrait>(txn: &C, version: u32) -> anyhow::Result<()> {
    let existing = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(txn)
        .await?;
    if let Some(row) = existing {
        let mut am: schema_version::ActiveModel = row.into();
        am.version = Set(version);
        am.update(txn).await?;
    } else {
        let am = schema_version::ActiveModel {
            id: Set(schema_version::Model::ROW_ID),
            version: Set(version),
        };
        am.insert(txn).await?;
    }
    Ok(())
}

fn read_redb_schema_version(db: &Arc<redb::Database>) -> anyhow::Result<u32> {
    let rdb = db.begin_read()?;
    match rdb.open_table(V3_TABLE_SCHEMA_VERSION) {
        Ok(t) => Ok(t.get(())?.map(|v| v.value()).unwrap_or(0)),
        Err(_) => Ok(0),
    }
}

fn read_all_playlists(db: &Arc<redb::Database>) -> anyhow::Result<Vec<v3::PlaylistModel>> {
    let rdb = db.begin_read()?;
    let t = rdb.open_table(V3_TABLE_PLAYLIST)?;
    let mut out = Vec::new();
    for row in t.iter()? {
        let (_, v) = row?;
        out.push(v.value());
    }
    Ok(out)
}

fn read_all_music(db: &Arc<redb::Database>) -> anyhow::Result<Vec<v3::MusicModel>> {
    let rdb = db.begin_read()?;
    let t = rdb.open_table(V3_TABLE_MUSIC)?;
    let mut out = Vec::new();
    for row in t.iter()? {
        let (_, v) = row?;
        out.push(v.value());
    }
    Ok(out)
}

fn read_all_storage(db: &Arc<redb::Database>) -> anyhow::Result<Vec<v3::StorageModel>> {
    let rdb = db.begin_read()?;
    let t = rdb.open_table(V3_TABLE_STORAGE)?;
    let mut out = Vec::new();
    for row in t.iter()? {
        let (_, v) = row?;
        out.push(v.value());
    }
    Ok(out)
}

fn read_all_playlist_music(
    db: &Arc<redb::Database>,
) -> anyhow::Result<Vec<(v3::PlaylistId, v3::MusicId)>> {
    let rdb = db.begin_read()?;
    let t = rdb.open_multimap_table(V3_TABLE_MUSIC_PLAYLIST)?;
    let mut out = Vec::new();
    for entry in t.iter()? {
        let (mid, playlists) = entry?;
        let mid_val = mid.value();
        for p in playlists {
            let pid = p?.value();
            out.push((pid, mid_val));
        }
    }
    Ok(out)
}

fn read_preference(db: &Arc<redb::Database>) -> anyhow::Result<Option<v3::PreferenceModel>> {
    let rdb = db.begin_read()?;
    if let Ok(t) = rdb.open_table(V3_TABLE_PREFERENCE) {
        if let Some(v) = t.get(())? {
            return Ok(Some(v.value()));
        }
    }
    Ok(None)
}

fn read_id_alloc(db: &Arc<redb::Database>) -> anyhow::Result<Vec<(v3::DbKeyAlloc, i64)>> {
    let rdb = db.begin_read()?;
    if let Ok(t) = rdb.open_table(V3_TABLE_ID_ALLOC) {
        let mut out = Vec::new();
        for row in t.iter()? {
            let (k, v) = row?;
            out.push((k.value(), v.value()));
        }
        return Ok(out);
    }
    Ok(Vec::new())
}

fn read_blob_next_id(db: &Arc<redb::Database>) -> anyhow::Result<Option<i64>> {
    let rdb = db.begin_read()?;
    if let Ok(t) = rdb.open_table(V3_TABLE_BLOB) {
        if let Some(v) = t.get(())? {
            // SelfType of `BinSerde<BlobId>` is `BlobId`.
            let bs: v3::BlobId = v.value();
            return Ok(Some(*bs.as_ref()));
        }
    }
    Ok(None)
}

// Silence unused-import warning when only some call sites are exercised.
#[allow(dead_code)]
fn _unused() {
    let _ = (playlist::Entity, music::Entity, storage::Entity, playlist_music::Entity, preference::Entity, id_alloc::Entity, blob::Entity);
}
