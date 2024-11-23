use ease_client_shared::backends::{
    music::MusicId, music_duration::MusicDuration, storage::StorageId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicModel {
    pub id: MusicId,
    pub path: String,
    pub storage_id: StorageId,
    pub title: String,
    pub duration: Option<MusicDuration>,
    pub cover: Option<Vec<u8>>,
    pub lyric_storage_id: Option<StorageId>,
    pub lyric_path: Option<String>,
    pub lyric_default: bool,
}
