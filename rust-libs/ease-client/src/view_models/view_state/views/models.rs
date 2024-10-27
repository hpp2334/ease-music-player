use super::{
    music::{VCurrentMusicLyricState, VCurrentMusicState, VTimeToPauseState},
    playlist::{
        VCreatePlaylistState, VCurrentPlaylistState, VEditPlaylistState, VPlaylistListState,
    },
    router::VRootSubKeyState,
    storage::{VCurrentStorageEntriesState, VEditStorageState, VStorageListState},
};

#[derive(Debug, Default, uniffi::Record)]
pub struct RootViewModelState {
    pub playlist_list: Option<VPlaylistListState>,
    pub current_playlist: Option<VCurrentPlaylistState>,
    pub edit_playlist: Option<VEditPlaylistState>,
    pub create_playlist: Option<VCreatePlaylistState>,
    pub storage_list: Option<VStorageListState>,
    pub current_storage_entries: Option<VCurrentStorageEntriesState>,
    pub edit_storage: Option<VEditStorageState>,
    pub current_music: Option<VCurrentMusicState>,
    pub time_to_pause: Option<VTimeToPauseState>,
    pub current_music_lyric: Option<VCurrentMusicLyricState>,
    pub current_router: Option<VRootSubKeyState>,
}
