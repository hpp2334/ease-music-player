use sea_orm::entity::prelude::*;

/// Storage **registry** — the universal table every music / playlist storage
/// reference (`music.loc_storage_id`, `music.lyric_storage_id`,
/// `playlist.picture_storage_id`) points at, regardless of the backing source
/// kind. `id` is the `StorageId` returned by `obtain(StorageHandle)`.
///
/// The `type` column is the `StorageType` discriminant; plugin rows carry
/// `plugin_id` + `plugin_storage_id`. Concrete connection details live in
/// each plugin's `plugin_kv` rows + the `secret` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "storage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// `StorageType` discriminant (Local=0, Plugin=2).
    #[sea_orm(column_name = "type")]
    pub r#type: i32,
    /// Set when `type = Plugin`; the plugin's manifest id
    /// (e.g. "com.ease.onedrive").
    pub plugin_id: Option<String>,
    /// Set when `type = Plugin`; the storage-contribution instance id within
    /// the plugin (e.g. "onedrive:1").
    pub plugin_storage_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
