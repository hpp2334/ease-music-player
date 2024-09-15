use serde::{Deserialize, Serialize};

use crate::define_id;

use super::music::MusicId;

define_id!(PlaylistId);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistModel {
    pub id: PlaylistId,
    pub title: String,
    pub created_time: i64,
    #[serde(with = "serde_bytes")]
    pub picture: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistMusicModel {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}
