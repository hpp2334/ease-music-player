use std::time::Duration;

use ease_client_schema::{DataSourceKey, PlaylistId, StorageEntryLoc};
use serde::{Deserialize, Serialize};

use super::music::MusicAbstract;

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub show_cover: Option<DataSourceKey>,
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub created_time: Duration,
    pub order: Vec<u32>,
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistAbstract {
    pub meta: PlaylistMeta,
    pub music_count: u64,
    #[serde_as(as = "Option<serde_with::DurationMilliSeconds<u64>>")]
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub abstr: PlaylistAbstract,
    pub musics: Vec<MusicAbstract>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
