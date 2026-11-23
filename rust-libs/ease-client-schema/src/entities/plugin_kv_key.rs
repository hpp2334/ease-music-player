use sea_orm::entity::prelude::*;

/// Key-registry table for plugin KV storage.
///
/// Each row is the unique (plugin_id, key) pair that a plugin has used. The
/// `kind` column tags whether the key is owned by the single-value table
/// (`Single = 0`, one value per key, overwrite semantics) or the multi-value
/// table (`Multi = 1`, append-only event log). A key is locked to its kind
/// on first use to prevent silent corruption from mixed-mode access.
///
/// The auto-increment `id` is the foreign key that `plugin_kv_single` and
/// `plugin_kv_multi` rows reference, so we never store the (plugin_id, key)
/// string pair on every value row — the join is by integer key_id.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "plugin_kv_key")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub plugin_id: String,
    pub key: String,
    /// Discriminant of `KvKind` (Single = 0, Multi = 1).
    pub kind: i32,
    /// Unix timestamp (ms) when the key was first registered.
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::plugin_kv_single::Entity")]
    PluginKvSingle,
    #[sea_orm(has_many = "super::plugin_kv_multi::Entity")]
    PluginKvMulti,
}

impl Related<super::plugin_kv_single::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PluginKvSingle.def()
    }
}

impl Related<super::plugin_kv_multi::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PluginKvMulti.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
