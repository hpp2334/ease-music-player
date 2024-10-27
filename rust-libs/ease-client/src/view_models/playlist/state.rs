
use ease_client_shared::{
    backends::{
        playlist::{Playlist, PlaylistAbstract, PlaylistId},
        storage::StorageEntryLoc,
    },
    uis::playlist::CreatePlaylistMode,
};

#[derive(Default, Clone)]
pub struct AllPlaylistState {
    pub playlists: Vec<PlaylistAbstract>,
}

#[derive(Default, Clone)]
pub struct CurrentPlaylistState {
    pub playlist: Option<Playlist>,
}

#[derive(Default, Clone)]
pub struct EditPlaylistState {
    pub id: Option<PlaylistId>,
    pub cover: Option<StorageEntryLoc>,
    pub playlist_name: String,
}

#[derive(Default, Clone)]
pub struct CreatePlaylistState {
    pub cover: Option<StorageEntryLoc>,
    pub playlist_name: String,
    pub entries: Vec<StorageEntryLoc>,
    pub mode: CreatePlaylistMode,
    pub recommend_playlist_names: Vec<String>,
}
