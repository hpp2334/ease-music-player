use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id, define_message};

use super::{
    music::{MusicId, MusicMeta},
    music_duration::MusicDuration,
    storage::StorageEntryLoc,
};

define_id!(PlaylistId);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover_url: String,
    pub created_time: Duration,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaylistAbstract {
    pub meta: PlaylistMeta,
    pub music_count: usize,
    pub duration: Option<MusicDuration>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Playlist {
    pub abstr: PlaylistAbstract,
    pub musics: Vec<MusicMeta>,
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
    pub fn cover_url(&self) -> &str {
        &self.meta.cover_url
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
    pub fn cover_url(&self) -> &str {
        self.abstr.cover_url()
    }
    pub fn duration(&self) -> &Option<MusicDuration> {
        &self.abstr.duration
    }
}

define_message!(
    GetAllPlaylistAbstractsMsg,
    Code::GetAllPlaylistMetas,
    (),
    Vec<PlaylistAbstract>
);

define_message!(
    GetPlaylistMsg,
    Code::GetPlaylist,
    PlaylistId,
    Option<Playlist>
);

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdatePlaylist {
    pub id: PlaylistId,
    pub title: String,
    pub picture: Option<StorageEntryLoc>,
    pub current_time_ms: i64,
}
define_message!(
    UpdatePlaylistMsg,
    Code::UpsertPlaylist,
    ArgUpdatePlaylist,
    ()
);

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgAddMusicsToPlaylist {
    pub id: PlaylistId,
    pub entries: Vec<(StorageEntryLoc, String)>,
}
define_message!(
    AddMusicsToPlaylistMsg,
    Code::AddMusicsToPlaylist,
    ArgAddMusicsToPlaylist,
    ()
);

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgRemoveMusicFromPlaylist {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}
define_message!(
    RemoveMusicsToPlaylistMsg,
    Code::RemoveMusicFromPlaylist,
    ArgRemoveMusicFromPlaylist,
    ()
);

define_message!(RemovePlaylistMsg, Code::RemovePlaylist, PlaylistId, ());
