use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::define_id;

use super::{
    music::{MusicAbstract, MusicId},
    music_duration::MusicDuration,
    storage::{DataSourceKey, StorageEntry, StorageEntryLoc},
};

define_id!(PlaylistId);

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode, Clone)]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub show_cover: Option<DataSourceKey>,
    pub created_time: Duration,
}

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode, Clone)]
pub struct PlaylistAbstract {
    pub meta: PlaylistMeta,
    pub music_count: usize,
    pub duration: Option<MusicDuration>,
}

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode, Clone)]
pub struct Playlist {
    pub abstr: PlaylistAbstract,
    pub musics: Vec<MusicAbstract>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
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
        &self.abstr.cover()
    }
    pub fn show_cover(&self) -> &Option<DataSourceKey> {
        self.abstr.show_cover()
    }
    pub fn duration(&self) -> &Option<MusicDuration> {
        &self.abstr.duration
    }
}

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct ArgUpdatePlaylist {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
}

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct ArgCreatePlaylist {
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub entries: Vec<(StorageEntry, String)>,
}
#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct ArgAddMusicsToPlaylist {
    pub id: PlaylistId,
    pub entries: Vec<(StorageEntry, String)>,
}

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct ArgRemoveMusicFromPlaylist {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}
