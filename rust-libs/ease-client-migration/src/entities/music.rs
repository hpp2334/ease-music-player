use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "music")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub loc_storage_id: i64,
    pub loc_path: String,
    pub title: String,
    /// Duration in milliseconds (NULL if unknown).
    pub duration_ms: Option<i64>,
    /// Foreign key into the blob id allocator; the actual bytes live on disk.
    pub cover_blob_id: Option<i64>,
    pub lyric_storage_id: Option<i64>,
    pub lyric_path: Option<String>,
    /// 0/1 boolean.
    pub lyric_default: i32,
    /// JSON-encoded `Vec<u32>` (ease-order-key raw).
    pub order: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::playlist_music::Entity")]
    PlaylistMusic,
}

impl Related<super::playlist_music::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlaylistMusic.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
