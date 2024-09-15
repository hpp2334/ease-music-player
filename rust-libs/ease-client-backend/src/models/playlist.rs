use ease_client_shared::{MusicId, PlaylistId, StorageId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistModel {
    pub id: PlaylistId,
    pub title: String,
    pub created_time: i64,
    pub picture_storage_id: Option<StorageId>,
    pub picture_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistMusicModel {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}
