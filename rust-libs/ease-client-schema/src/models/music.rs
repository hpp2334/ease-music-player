use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::shared::{BlobId, MusicId, StorageEntryLoc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicModel {
    pub id: MusicId,
    pub loc: StorageEntryLoc,
    pub title: String,
    /// Track artist from container tags (empty = never probed / no tag).
    pub artist: String,
    pub duration: Option<Duration>,
    pub cover: Option<BlobId>,
    pub lyric: Option<StorageEntryLoc>,
    pub lyric_default: bool,
    /// Embedded lyric text captured from container tags (None = never
    /// probed / no tag).
    pub embedded_lyric: Option<String>,
    pub order: Vec<u32>,
}
