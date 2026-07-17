use sea_orm::entity::prelude::*;

/// Join table replacing the legacy `playlist_music` and `music_playlist`
/// multimimap tables. Each row is one (playlist, music) edge.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playlist_music")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub playlist_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub music_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::playlist::Entity",
        from = "Column::PlaylistId",
        to = "super::playlist::Column::Id"
    )]
    Playlist,
    #[sea_orm(
        belongs_to = "super::music::Entity",
        from = "Column::MusicId",
        to = "super::music::Column::Id"
    )]
    Music,
}

impl Related<super::playlist::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Playlist.def()
    }
}

impl Related<super::music::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Music.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
