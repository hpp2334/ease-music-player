use misty_vm::views::MistyViewModelManagerBuilder;

use crate::{
    modules::{music::views::*, playlist::views::*, router::views::*, storage::views::*},
    RootViewModelState,
};

pub fn with_view_models(
    builder: MistyViewModelManagerBuilder<RootViewModelState>,
) -> MistyViewModelManagerBuilder<RootViewModelState> {
    let builder = register_storage_viewmodels(builder);
    let builder = register_playlist_viewmodels(builder);
    // Music
    let builder = builder
        .register(current_music_view_model)
        .register(time_to_pause_view_model)
        .register(current_music_lyric_view_model)
        // Router
        .register(root_subkey_view_model);
    builder
}
