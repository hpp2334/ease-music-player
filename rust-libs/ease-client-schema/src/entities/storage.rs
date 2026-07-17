use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "storage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    /// 0/1 boolean.
    pub is_anonymous: i32,
    /// Discriminant of `StorageType` enum (Local=0, Webdav=1, OneDrive=2).
    pub typ: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
