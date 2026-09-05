use sea_orm::entity::prelude::*;

/// Single-row table (id = 0) holding user preferences.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "preference")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    /// Discriminant of `PlayMode` enum (Single=0, SingleLoop=1, List=2, ListLoop=3).
    pub playmode: i32,
    /// BCP-47 tag of the in-app language override (e.g. "zh-CN");
    /// `None` = follow the system locale.
    pub language: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
