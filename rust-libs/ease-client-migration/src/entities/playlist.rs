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
