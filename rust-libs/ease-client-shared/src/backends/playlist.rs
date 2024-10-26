use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id, define_message};

use super::{
    music::{MusicAbstract, MusicId, MusicMeta},
    music_duration::MusicDuration,
    storage::StorageEntryLoc,
};

define_id!(PlaylistId);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
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
    pub musics: Vec<MusicAbstract>,
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
    pub cover: Option<StorageEntryLoc>,
}
define_message!(
    UpdatePlaylistMsg,
    Code::UpdatePlaylist,
    ArgUpdatePlaylist,
    ()
);

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgCreatePlaylist {
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub entries: Vec<StorageEntryLoc>,
}
define_message!(
    CreatePlaylistMsg,
    Code::CreatePlaylist,
    ArgCreatePlaylist,
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
    RemoveMusicsFromPlaylistMsg,
    Code::RemoveMusicFromPlaylist,
    ArgRemoveMusicFromPlaylist,
    ()
);

define_message!(RemovePlaylistMsg, Code::RemovePlaylist, PlaylistId, ());
