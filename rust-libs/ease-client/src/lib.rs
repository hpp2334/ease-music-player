mod api;
mod core_views;
pub mod modules;
pub(crate) mod utils;

use core_views::with_view_models;
use ease_client_shared::uis::view::RootViewModelState;
pub use misty_vm::resources::ResourceUpdateAction;
pub use misty_vm::{
    client::{MistyClientHandle, SingletonMistyClientPod},
    controllers::MistyController,
    resources::{MistyResourceHandle, MistyResourceId, MistyResourceManager},
    services::MistyServiceManager,
    signals::MistySignal,
    views::MistyViewModelManager,
};
use misty_vm::{misty_states, states::MistyStateManager};

use crate::modules::{
    music::service::{CurrentMusicState, TimeToPauseState},
    playlist::service::{
        AllPlaylistState, CreatePlaylistState, CurrentPlaylistState, EditPlaylistState,
    },
    router::service::RouterState,
    storage::service::{CurrentStorageState, EditStorageState, StoragesRecordState, StoragesState},
    PreferenceState,
};

uniffi::setup_scaffolding!();

pub fn build_view_manager() -> MistyViewModelManager<RootViewModelState> {
    let view_manager = with_view_models(MistyViewModelManager::builder()).build();
    view_manager
}

pub fn build_state_manager() -> MistyStateManager {
    MistyStateManager::new(misty_states!(
        // Music
        CurrentMusicState,
        TimeToPauseState,
        // Playlist
        AllPlaylistState,
        CurrentPlaylistState,
        EditPlaylistState,
        CreatePlaylistState,
        // Preference
        PreferenceState,
        // Router
        RouterState,
        // Storage
        StoragesState,
        CurrentStorageState,
        StoragesRecordState,
        EditStorageState
    ))
}
