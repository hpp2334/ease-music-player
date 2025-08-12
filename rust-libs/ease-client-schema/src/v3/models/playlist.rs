use serde::{Deserialize, Serialize};

use super::super::objects::{MusicId, PlaylistId, StorageEntryLoc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistModel {
    pub id: PlaylistId,
    pub title: String,
    pub created_time: i64,
    pub picture: Option<StorageEntryLoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistMusicModel {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}
