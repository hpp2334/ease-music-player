use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playlist")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
    pub created_time: i64,
    pub picture_storage_id: Option<i64>,
    pub picture_path: Option<String>,
    /// JSON-encoded `Vec<u32>` (ease-order-key raw).
    pub order: String,
    /// JSON-encoded `Vec<i64>` of storage row ids; NULL = allow all
    /// storages (see `PlaylistModel::storage_allowlist`).
    pub storage_allowlist: Option<String>,
    /// Owning `playlist_group` row id. NULL only transiently (pre-group
    /// databases) — the runtime ensure-default sweep reassigns orphans
    /// to the first group.
    pub group_id: Option<i64>,
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
