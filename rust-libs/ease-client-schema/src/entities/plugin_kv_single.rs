use sea_orm::entity::prelude::*;

/// Single-value plugin KV storage. At most one row per `key_id` (enforced by
/// the `key_id` primary key). `set` is an upsert; `get` is a direct lookup.
///
/// `key_id` references `plugin_kv_key.id` and is also this table's primary
/// key — the uniqueness constraint is structural.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "plugin_kv_single")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub key_id: i64,
    pub value: String,
    /// Unix timestamp (ms) of the last upsert.
    pub updated_at: i64,
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
