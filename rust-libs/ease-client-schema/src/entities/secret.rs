use sea_orm::entity::prelude::*;

/// An opaque secret (e.g. a WebDAV password or a plugin refresh token), scoped
/// to an owner via the `scope` column:
///   - `"internal"`         — host-owned (e.g. a WebDAV password).
///   - `"plugin:<plugin_id>"` — owned by that plugin.
///
/// Access is enforced by the `SecretStore` service: a caller may only
/// `get` / `remove` a secret whose scope matches its own. On Android this
/// table backs the store; on other platforms the same `SecretStore` trait
/// can be backed by the platform keychain.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "secret")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub scope: String,
    pub secret: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
