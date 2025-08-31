use crate::v2::*;

#[derive(Debug, Clone, Copy, bitcode::Encode, bitcode::Decode, PartialEq, Eq, PartialOrd, Ord)]
pub enum DbKeyAlloc {
    Playlist,
    Music,
    Storage,
}

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

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct PlaylistModel {
    pub id: PlaylistId,
    pub title: String,
    pub created_time: i64,
    pub picture: Option<StorageEntryLoc>,
}

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct PlaylistMusicModel {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct StorageModel {
    pub id: StorageId,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}
