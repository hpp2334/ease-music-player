use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::super::objects::{BlobId, MusicId, StorageEntryLoc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicModel {
    pub id: MusicId,
    pub loc: StorageEntryLoc,
    pub title: String,
    pub duration: Option<Duration>,
    pub cover: Option<BlobId>,
    pub lyric: Option<StorageEntryLoc>,
    pub lyric_default: bool,
}
