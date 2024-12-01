use ease_client_shared::backends::{
    music::MusicId, playlist::PlaylistId, storage::StorageEntryLoc,
};

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
