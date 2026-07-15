use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(Storage::Addr).text().not_null().default(""))
                    .col(ColumnDef::new(Storage::Alias).text().not_null().default(""))
                    .col(ColumnDef::new(Storage::Username).text().not_null().default(""))
                    .col(ColumnDef::new(Storage::Password).text().not_null().default(""))
                    .col(
                        ColumnDef::new(Storage::IsAnonymous)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Storage::Typ).integer().not_null().default(1))
                    .to_owned(),
            )
            .await?;

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
                    .to_owned(),
            )
            .await?;

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

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_query::IntoIden;
        let tables: Vec<sea_query::DynIden> = vec![
            BlobTbl::Table.into_iden(),
            IdAlloc::Table.into_iden(),
            SchemaVersion::Table.into_iden(),
            Preference::Table.into_iden(),
            PlaylistMusic::Table.into_iden(),
            Music::Table.into_iden(),
            Playlist::Table.into_iden(),
            Storage::Table.into_iden(),
        ];
        for t in tables {
            manager
                .drop_table(Table::drop().table(t).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Storage {
    Table,
    Id,
    Addr,
    Alias,
    Username,
    Password,
    IsAnonymous,
    Typ,
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
