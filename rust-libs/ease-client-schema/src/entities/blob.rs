use sea_orm::entity::prelude::*;

/// Single-row counter table (id = 0) backing the filesystem `BlobManager`
/// next-blob-id allocator. Only the integer lives here; bytes are on disk.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "blob")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    /// Next blob id to allocate.
    pub next_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub const ROW_ID: i32 = 0;
}
