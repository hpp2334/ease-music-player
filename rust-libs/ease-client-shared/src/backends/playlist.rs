use misty_serve::define_message;
use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id};

use super::{
    music::{MusicId, MusicMeta},
    storage::StorageEntryLoc,
};

define_id!(PlaylistId);

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover_loc: Option<StorageEntryLoc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub meta: PlaylistMeta,
    pub musics: Vec<MusicMeta>,
}

define_message!(
    GetAllPlaylistMetasMsg,
    Code::GetAllPlaylistMetas,
    (),
    Vec<PlaylistMeta>
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
