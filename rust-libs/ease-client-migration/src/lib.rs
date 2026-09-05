use std::path::PathBuf;
use std::sync::Arc;

use crate::legacy::schema_v3 as v3;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait};
use sea_orm_migration::MigratorTrait;

pub mod converter;
pub(crate) mod legacy;
pub mod migrations;

pub use sea_orm::DatabaseConnection as DbConn;

use ease_client_schema::entities::{
    blob, id_alloc, music, playlist, playlist_music, plugin_kv_key, plugin_kv_single, preference,
    schema_version, secret, storage,
};
use crate::legacy::redb_v3::TABLE_BLOB as V3_TABLE_BLOB;
use crate::legacy::redb_v3::TABLE_ID_ALLOC as V3_TABLE_ID_ALLOC;
use crate::legacy::redb_v3::TABLE_MUSIC as V3_TABLE_MUSIC;
use crate::legacy::redb_v3::TABLE_MUSIC_PLAYLIST as V3_TABLE_MUSIC_PLAYLIST;
use crate::legacy::redb_v3::TABLE_PLAYLIST as V3_TABLE_PLAYLIST;
use crate::legacy::redb_v3::TABLE_PREFERENCE as V3_TABLE_PREFERENCE;
use crate::legacy::redb_v3::TABLE_STORAGE as V3_TABLE_STORAGE;
use crate::legacy::redb_v3::TABLE_SCHEMA_VERSION as V3_TABLE_SCHEMA_VERSION;
use crate::legacy::{upgrade_v1_to_v2, upgrade_v2_to_v3};
use redb::{ReadableMultimapTable, ReadableTable};

/// Plugin ids owning OneDrive / WebDAV storage instances (matching the
/// plugins' manifests). Used when migrating legacy rows into the secret
/// table + plugin_kv.
const ONEDRIVE_PLUGIN_ID: &str = "com.ease.onedrive";
const WEBDAV_PLUGIN_ID: &str = "com.ease.webdav";

/// The schema version produced by this crate.
pub const SCHEMA_VERSION: u32 = 7;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        vec![
            Box::new(migrations::InitMigration),
            Box::new(migrations::PluginKvMigration),
            Box::new(migrations::StorageRegistryMigration),
            Box::new(migrations::WebdavPluginMigration),
            Box::new(migrations::PreferenceLanguageMigration),
        ]
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
        converter::playlist_from(m.into()).insert(&txn).await?;
    }

    let music_rows = read_all_music(&redb_db)?;
    for m in music_rows {
        converter::music_from(m.into()).insert(&txn).await?;
    }

    let storage_rows = read_all_storage(&redb_db)?;
    // The sea-orm migrations already ran (new-shape tables + a placeholder
    // Local registry row seeded for fresh installs). redb is authoritative on
    // this path, so wipe the placeholder registry/detail rows before importing.
    storage::Entity::delete_many().exec(&txn).await?;
    secret::Entity::delete_many().exec(&txn).await?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    for m in storage_rows {
        import_legacy_storage_row(&txn, m, now_ms).await?;
    }

    let playlist_music_rows = read_all_playlist_music(&redb_db)?;
    for (pid, mid) in playlist_music_rows {
        converter::playlist_music_from(pid.into(), mid.into()).insert(&txn).await?;
    }

    if let Some(pref) = read_preference(&redb_db)? {
        converter::preference_from(pref.into()).insert(&txn).await?;
    }

    for (kind, next_id) in read_id_alloc(&redb_db)? {
        converter::id_alloc_from(kind.into(), next_id).insert(&txn).await?;
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

/// Import one legacy (v3) storage row into the new-shape tables. Local ->
/// registry row; Webdav / OneDrive -> plugin-scoped secret + plugin_kv
/// instance record + plugin registry row (WebDAV's connection fields live in
/// the kv value). Explicit `id`s preserve music/playlist refs.
async fn import_legacy_storage_row<C: sea_orm::ConnectionTrait>(
    txn: &C,
    m: v3::StorageModel,
    now_ms: i64,
) -> anyhow::Result<()> {
    use sea_orm::ActiveValue::Set;
    let id = *m.id.as_ref();

    match m.typ {
        v3::StorageType::Local => {
            let am = storage::ActiveModel {
                id: Set(id),
                r#type: Set(0),
                plugin_id: Set(None),
                plugin_storage_id: Set(None),
            };
            am.insert(txn).await?;
        }
        v3::StorageType::Webdav => {
            let secret_id = if m.password.is_empty() {
                None
            } else {
                let am = secret::ActiveModel {
                    id: sea_orm::ActiveValue::NotSet,
                    scope: Set(format!("plugin:{}", WEBDAV_PLUGIN_ID)),
                    secret: Set(m.password.clone()),
                };
                Some(am.insert(txn).await?.id)
            };
            let instance = format!("webdav:{}", id);
            let instance_key = format!("storage:{}", instance);
            let key_am = plugin_kv_key::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                plugin_id: Set(WEBDAV_PLUGIN_ID.to_string()),
                key: Set(instance_key),
                kind: Set(0),
                created_at: Set(now_ms),
            };
            let key_model = key_am.insert(txn).await?;
            let value = serde_json::json!({
                "alias": m.alias,
                "secretId": secret_id,
                "addr": m.addr,
                "username": m.username,
                "isAnonymous": m.is_anonymous,
            })
            .to_string();
            let single_am = plugin_kv_single::ActiveModel {
                key_id: Set(key_model.id),
                value: Set(value),
                updated_at: Set(now_ms),
            };
            single_am.insert(txn).await?;
            let reg = storage::ActiveModel {
                id: Set(id),
                r#type: Set(2),
                plugin_id: Set(Some(WEBDAV_PLUGIN_ID.to_string())),
                plugin_storage_id: Set(Some(instance)),
            };
            reg.insert(txn).await?;
        }
        v3::StorageType::OneDrive => {
            let secret_id = if m.password.is_empty() {
                None
            } else {
                let am = secret::ActiveModel {
                    id: sea_orm::ActiveValue::NotSet,
                    scope: Set(format!("plugin:{}", ONEDRIVE_PLUGIN_ID)),
                    secret: Set(m.password.clone()),
                };
                Some(am.insert(txn).await?.id)
            };
            let instance_key = format!("storage:onedrive:{}", id);
            let key_am = plugin_kv_key::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                plugin_id: Set(ONEDRIVE_PLUGIN_ID.to_string()),
                key: Set(instance_key),
                kind: Set(0),
                created_at: Set(now_ms),
            };
            let key_model = key_am.insert(txn).await?;
            let value = serde_json::json!({ "alias": m.alias, "secretId": secret_id })
                .to_string();
            let single_am = plugin_kv_single::ActiveModel {
                key_id: Set(key_model.id),
                value: Set(value),
                updated_at: Set(now_ms),
            };
            single_am.insert(txn).await?;
            let reg = storage::ActiveModel {
                id: Set(id),
                r#type: Set(2),
                plugin_id: Set(Some(ONEDRIVE_PLUGIN_ID.to_string())),
                plugin_storage_id: Set(Some(format!("onedrive:{}", id))),
            };
            reg.insert(txn).await?;
        }
    }
    Ok(())
}
