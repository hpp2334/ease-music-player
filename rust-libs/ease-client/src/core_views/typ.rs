use crate::modules::music::typ::*;
use crate::modules::playlist::typ::*;
use crate::modules::storage::typ::*;
use crate::modules::VRootSubKeyState;

macro_rules! impl_root_view_state_merge_from {
    ($s:ident, $($field:tt,$typ:ident),*) => {
        #[derive(Debug, Default, Clone)]
        pub struct $s {
            $(pub $field: Option<$typ>),*
        }
        impl $s {
            pub fn merge_from(&mut self, rhs: &Self) {
                $(
                    if rhs.$field.is_some() {
                        self.$field = rhs.$field.clone();
                    }
                )*
            }
        }
    };
}
impl_root_view_state_merge_from!(
    RootViewModelState,
    // Playlist
    playlist_list,
    VPlaylistListState,
    current_playlist,
    VCurrentPlaylistState,
    edit_playlist,
    VEditPlaylistState,
    create_playlist,
    VCreatePlaylistState,
    // Storage
    storage_list,
    VStorageListState,
    current_storage_entries,
    VCurrentStorageEntriesState,
    edit_storage,
    VEditStorageState,
    // Music
    current_music,
    VCurrentMusicState,
    time_to_pause,
    VTimeToPauseState,
    current_music_lyric,
    VCurrentMusicLyricState,
    // Router
    current_router,
    VRootSubKeyState
);
