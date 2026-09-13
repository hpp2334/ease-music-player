use sea_orm::entity::prelude::*;

/// A named, ordered container of playlists (`playlist.group_id` points
/// here). `expanded` persists the UI expand/collapse state per group —
/// it is written on every user toggle and defaults to expanded (1).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playlist_group")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
    pub created_time: i64,
    /// JSON-encoded `Vec<u32>` (ease-order-key raw).
    pub order: String,
    /// 1 = expanded, 0 = collapsed (see `PlaylistGroupModel::expanded`).
    pub expanded: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
