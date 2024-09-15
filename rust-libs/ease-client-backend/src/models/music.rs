use std::{ops::Deref, time::Duration};

use ease_client_shared::{MusicDuration, MusicId, StorageId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicModel {
    pub id: MusicId,
    pub path: String,
    pub storage_id: StorageId,
    pub title: String,
    pub duration: Option<MusicDuration>,
    pub picture_storage_id: Option<StorageId>,
    pub picture_path: Option<String>,
    pub lyric_storage_id: Option<StorageId>,
    pub lyric_path: Option<String>,
}
