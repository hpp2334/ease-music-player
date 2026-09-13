//! The single v4 init: creates the full SQLite schema in one step.
//!
//! Version lineage: v1–v3 are the legacy `redb` formats (upgraded in-place
//! by the `legacy/` upgraders before `import_from_redb` streams rows into
//! these tables); **v4 is the SQLite schema** produced here. The dev-era
//! internal v4-line migrations (plugin-kv, storage-registry split,
//! webdav-to-plugin, preference-language — schema versions 5–7) were folded
//! back into this init: v0.4 never shipped, so the only supported upgrade
//! path is v3 (redb) -> v4 (this schema).
//!
//! The migration name (`m20260715_000001_init`, derived from the module
//! path) is deliberately kept from the original first migration, so
//! databases that already ran the old chain have it recorded in
//! `seaql_migrations`, skip this body, and keep working — their end state
//! is byte-for-byte the schema below.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---------- storage registry ----------
        manager
            .create_table(
                Table::create()
                    .table(Storage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Storage::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Storage::Typ).integer().not_null())
                    .col(ColumnDef::new(Storage::PluginId).text().null())
                    .col(ColumnDef::new(Storage::PluginStorageId).text().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_storage_plugin")
                    .table(Storage::Table)
                    .col(Storage::PluginId)
                    .col(Storage::PluginStorageId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ---------- playlists / musics ----------
        manager
            .create_table(
                Table::create()
                    .table(Playlist::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Playlist::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Playlist::Title).text().not_null().default(""))
                    .col(
                        ColumnDef::new(Playlist::CreatedTime)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Playlist::PictureStorageId).big_integer().null())
                    .col(ColumnDef::new(Playlist::PicturePath).text().null())
                    .col(ColumnDef::new(Playlist::Order).text().not_null().default("[]"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Music::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Music::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Music::LocStorageId).big_integer().not_null())
                    .col(ColumnDef::new(Music::LocPath).text().not_null())
                    .col(ColumnDef::new(Music::Title).text().not_null().default(""))
                    .col(ColumnDef::new(Music::DurationMs).big_integer().null())
                    .col(ColumnDef::new(Music::CoverBlobId).big_integer().null())
                    .col(ColumnDef::new(Music::LyricStorageId).big_integer().null())
                    .col(ColumnDef::new(Music::LyricPath).text().null())
                    .col(
                        ColumnDef::new(Music::LyricDefault)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(ColumnDef::new(Music::Order).text().not_null().default("[]"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_music_loc")
                    .table(Music::Table)
                    .col(Music::LocStorageId)
                    .col(Music::LocPath)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PlaylistMusic::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlaylistMusic::PlaylistId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaylistMusic::MusicId)
                            .big_integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(PlaylistMusic::PlaylistId)
                            .col(PlaylistMusic::MusicId),
                    )
                    .to_owned(),
            )
            .await?;

        // ---------- preference ----------
        manager
            .create_table(
                Table::create()
                    .table(Preference::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Preference::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Preference::Playmode)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Preference::Language).text().null())
                    .to_owned(),
            )
            .await?;

        // ---------- bookkeeping ----------
        manager
            .create_table(
                Table::create()
                    .table(SchemaVersion::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SchemaVersion::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SchemaVersion::Version)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IdAlloc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IdAlloc::Kind)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(IdAlloc::NextId)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(BlobTbl::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlobTbl::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlobTbl::NextId)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        // ---------- plugin KV + secrets (raw SQL, same rationale as the
        // statements folded in from the former migrations: statement shapes
        // identical to what `sqlite3` would run) ----------
        manager
            .get_connection()
            .execute_unprepared(
                "
                CREATE TABLE IF NOT EXISTS plugin_kv_key (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    plugin_id   TEXT    NOT NULL,
                    key         TEXT    NOT NULL,
                    kind        INTEGER NOT NULL,
                    created_at  INTEGER NOT NULL,
                    UNIQUE (plugin_id, key)
                );
                CREATE INDEX IF NOT EXISTS idx_plugin_kv_key_plugin
                    ON plugin_kv_key (plugin_id);

                CREATE TABLE IF NOT EXISTS plugin_kv_single (
                    key_id      INTEGER PRIMARY KEY,
                    value       TEXT    NOT NULL,
                    updated_at  INTEGER NOT NULL,
                    FOREIGN KEY (key_id) REFERENCES plugin_kv_key(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS plugin_kv_multi (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    key_id      INTEGER NOT NULL,
                    value       TEXT    NOT NULL,
                    created_at  INTEGER NOT NULL,
                    FOREIGN KEY (key_id) REFERENCES plugin_kv_key(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_plugin_kv_multi_key_id
                    ON plugin_kv_multi (key_id, id);

                CREATE TABLE IF NOT EXISTS secret (
                    id      INTEGER PRIMARY KEY AUTOINCREMENT,
                    scope   TEXT    NOT NULL DEFAULT 'internal',
                    secret  TEXT    NOT NULL DEFAULT ''
                );
                ",
            )
            .await?;

        // ---------- seed the Local registry row ----------
        // Guarded so re-running against an already-populated registry is a
        // no-op (`import_from_redb` wipes and re-imports storage anyway).
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO storage (type) \
                 SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM storage WHERE type = 0)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Children before parents (plugin_kv_* reference plugin_kv_key);
        // dropping a table drops its indexes.
        manager
            .get_connection()
            .execute_unprepared(
                "
                DROP TABLE IF EXISTS plugin_kv_multi;
                DROP TABLE IF EXISTS plugin_kv_single;
                DROP TABLE IF EXISTS plugin_kv_key;
                DROP TABLE IF EXISTS secret;
                DROP TABLE IF EXISTS blob;
                DROP TABLE IF EXISTS id_alloc;
                DROP TABLE IF EXISTS schema_version;
                DROP TABLE IF EXISTS preference;
                DROP TABLE IF EXISTS playlist_music;
                DROP TABLE IF EXISTS music;
                DROP TABLE IF EXISTS playlist;
                DROP TABLE IF EXISTS storage;
                ",
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Storage {
    Table,
    Id,
    /// Named `Typ` (not `Type`) to avoid colliding with the Rust keyword;
    /// the `#[sea_orm(iden = "type")]` attribute makes the on-disk column
    /// name match the SeaORM entity's `column_name = "type"`.
    #[sea_orm(iden = "type")]
    Typ,
    PluginId,
    PluginStorageId,
}

#[derive(DeriveIden)]
enum Playlist {
    Table,
    Id,
    Title,
    CreatedTime,
    PictureStorageId,
    PicturePath,
    Order,
}

#[derive(DeriveIden)]
enum Music {
    Table,
    Id,
    LocStorageId,
    LocPath,
    Title,
    DurationMs,
    CoverBlobId,
    LyricStorageId,
    LyricPath,
    LyricDefault,
    Order,
}

#[derive(DeriveIden)]
enum PlaylistMusic {
    Table,
    PlaylistId,
    MusicId,
}

#[derive(DeriveIden)]
enum Preference {
    Table,
    Id,
    Playmode,
    Language,
}

#[derive(DeriveIden)]
enum SchemaVersion {
    Table,
    Id,
    Version,
}

#[derive(DeriveIden)]
enum IdAlloc {
    Table,
    Kind,
    NextId,
}

/// Named `BlobTbl` (not `Blob`) to avoid colliding with the sea-query
/// `Blob` SQL type re-exported by `sea_orm_migration::prelude::*`. The
/// `#[sea_orm(iden = "blob")]` attribute makes the on-disk table name match
/// the SeaORM entity's `table_name = "blob"`.
#[derive(DeriveIden)]
enum BlobTbl {
    #[sea_orm(iden = "blob")]
    Table,
    Id,
    NextId,
}
