use super::{
    music::{VCurrentMusicLyricState, VCurrentMusicState, VTimeToPauseState},
    playlist::{
        VCreatePlaylistState, VCurrentPlaylistState, VEditPlaylistState, VPlaylistListState,
    },
    router::VRootSubKeyState,
    storage::{VCurrentStorageEntriesState, VEditStorageState, VStorageListState},
};

macro_rules! generate_vs {
    ($struct_name:ident, $( $field_name:ident, $field_type:ty ),* ) => {
        #[derive(Debug, Clone, Default, uniffi::Record)]
        pub struct $struct_name {
            $( pub $field_name: Option<$field_type>, )*
        }

        impl $struct_name {
            pub fn merge_from(&mut self, other: $struct_name) {
                $(
                    if self.$field_name.is_none() && other.$field_name.is_some() {
                        self.$field_name = other.$field_name;
                    }
                )*
            }
        }
    };
}

generate_vs!(
    RootViewModelState,
    playlist_list,
    VPlaylistListState,
    current_playlist,
    VCurrentPlaylistState,
    edit_playlist,
    VEditPlaylistState,
    create_playlist,
    VCreatePlaylistState,
    storage_list,
    VStorageListState,
    current_storage_entries,
    VCurrentStorageEntriesState,
    edit_storage,
    VEditStorageState,
    current_music,
    VCurrentMusicState,
    time_to_pause,
    VTimeToPauseState,
    current_music_lyric,
    VCurrentMusicLyricState,
    current_router,
    VRootSubKeyState
);
