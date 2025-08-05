use std::time::Duration;

use crate::define_id;

use super::{
    music::MusicAbstract,
    storage::{DataSourceKey, StorageEntryLoc},
};

define_id!(PlaylistId);

#[derive(Debug, Clone, uniffi::Record)]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub show_cover: Option<DataSourceKey>,
    pub created_time: Duration,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PlaylistAbstract {
    pub meta: PlaylistMeta,
    pub music_count: u64,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Playlist {
    pub abstr: PlaylistAbstract,
    pub musics: Vec<MusicAbstract>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CreatePlaylistMode {
    #[default]
    Full,
    Empty,
}

impl PlaylistAbstract {
    pub fn id(&self) -> PlaylistId {
        self.meta.id
    }
    pub fn title(&self) -> &str {
        &self.meta.title
    }
    pub fn created_time(&self) -> &Duration {
        &self.meta.created_time
    }
    pub fn cover(&self) -> &Option<StorageEntryLoc> {
        &self.meta.cover
    }
    pub fn show_cover(&self) -> &Option<DataSourceKey> {
        &self.meta.show_cover
    }
}

impl Playlist {
    pub fn id(&self) -> PlaylistId {
        self.abstr.meta.id
    }
    pub fn title(&self) -> &str {
        self.abstr.title()
    }
    pub fn created_time(&self) -> &Duration {
        self.abstr.created_time()
    }
    pub fn cover(&self) -> &Option<StorageEntryLoc> {
        self.abstr.cover()
    }
    pub fn show_cover(&self) -> &Option<DataSourceKey> {
        self.abstr.show_cover()
    }
    pub fn duration(&self) -> &Option<Duration> {
        &self.abstr.duration
    }
}
