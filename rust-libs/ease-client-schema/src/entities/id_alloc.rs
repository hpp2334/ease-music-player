use sea_orm::entity::prelude::*;

/// Legacy ID allocator table. Populated only during v3 -> v4 import so the
/// SQLite schema can reflect the historical `id_alloc` rows. New rows use
/// SQLite autoincrement and do not consult this table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "id_alloc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    /// Discriminant of `DbKeyAlloc` (Playlist=0, Music=1, Storage=2).
    pub kind: i32,
    pub next_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
