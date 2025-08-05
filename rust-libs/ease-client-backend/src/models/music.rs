use std::time::Duration;

use crate::objects::{BlobId, MusicId, StorageEntryLoc};

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct MusicModel {
    pub id: MusicId,
    pub loc: StorageEntryLoc,
    pub title: String,
    pub duration: Option<Duration>,
    pub cover: Option<BlobId>,
    pub lyric: Option<StorageEntryLoc>,
    pub lyric_default: bool,
}

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct LegacyMusicModelV2 {
    pub id: MusicId,
    pub loc: StorageEntryLoc,
    pub title: String,
    pub duration: Option<LegacyMusicDurationV2>,
    pub cover: Option<BlobId>,
    pub lyric: Option<StorageEntryLoc>,
    pub lyric_default: bool,
}

#[derive(Debug, Clone, Copy, bitcode::Encode, bitcode::Decode)]
pub struct LegacyMusicDurationV2(Duration);

impl From<LegacyMusicModelV2> for MusicModel {
    fn from(legacy: LegacyMusicModelV2) -> Self {
        MusicModel {
            id: legacy.id,
            loc: legacy.loc,
            title: legacy.title,
            duration: legacy.duration.map(|d| d.0),
            cover: legacy.cover,
            lyric: legacy.lyric,
            lyric_default: legacy.lyric_default,
        }
    }
}
