use sea_orm::entity::prelude::*;

/// WebDAV connection details for a `storage` registry row of kind `Webdav`.
/// The password is NOT stored here — it lives in the `secret` table
/// (scope = "internal") and is referenced via `secret_id`.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "webdav_storage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub addr: String,
    pub alias: String,
    pub username: String,
    /// FK -> `secret.id`. NULL for anonymous storages.
    pub secret_id: Option<i64>,
    /// 0/1 boolean.
    pub is_anonymous: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
