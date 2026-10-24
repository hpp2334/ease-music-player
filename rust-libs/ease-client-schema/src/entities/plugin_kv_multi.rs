use sea_orm::entity::prelude::*;

/// Multi-value (append-only) plugin KV storage. Each `append` call adds a
/// new row keyed by `key_id`; multiple rows per `key_id` are expected.
/// Insertion order is preserved by the auto-increment `id` and backed by
/// the `(key_id, id)` index for efficient ordered scans.
///
/// Use this for event-log style data — e.g. play-count records one row per
/// play event, and the view aggregates by counting rows per `key_id`.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "plugin_kv_multi")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub key_id: i64,
    pub value: String,
    /// Unix timestamp (ms) when the row was appended.
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugin_kv_key::Entity",
        from = "Column::KeyId",
        to = "super::plugin_kv_key::Column::Id"
    )]
    PluginKvKey,
}

impl Related<super::plugin_kv_key::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PluginKvKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
