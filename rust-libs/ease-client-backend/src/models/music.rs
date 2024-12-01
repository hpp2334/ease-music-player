use ease_client_shared::backends::{
    music::MusicId,
    music_duration::MusicDuration,
    storage::{BlobId, StorageEntryLoc},
};

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct MusicModel {
    pub id: MusicId,
    pub loc: StorageEntryLoc,
    pub title: String,
    pub duration: Option<MusicDuration>,
    pub cover: Option<BlobId>,
    pub lyric: Option<StorageEntryLoc>,
    pub lyric_default: bool,
}
