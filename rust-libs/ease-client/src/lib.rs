mod api;
mod core_views;
pub mod modules;
pub(crate) mod utils;

use core_views::with_view_models;
pub use core_views::RootViewModelState;
use ease_client_shared::uniffi;
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
    app::service::GlobalAppState,
    music::service::{
        CachedMusicCoverHandlesState, CurrentMusicAssetState, CurrentMusicState, TimeToPauseState,
    },
    playlist::service::{
        AllPlaylistState, CreatePlaylistState, CurrentPlaylistState, EditPlaylistState,
    },
    router::service::RouterState,
    server::service::CurrentServerState,
    storage::service::{
        CurrentStorageState, EditStorageState, StorageBackendStaticState, StoragesRecordState,
        StoragesState,
    },
    PreferenceState,
};

uniffi::setup_scaffolding!();

pub fn build_view_manager() -> MistyViewModelManager<RootViewModelState> {
    let view_manager = with_view_models(MistyViewModelManager::builder()).build();
    view_manager
}

pub fn build_state_manager() -> MistyStateManager {
    MistyStateManager::new(misty_states!(
        // App
        GlobalAppState,
        // Music
        CurrentMusicState,
        CachedMusicCoverHandlesState,
        TimeToPauseState,
        CurrentMusicAssetState,
        // Playlist
        AllPlaylistState,
        CurrentPlaylistState,
        EditPlaylistState,
        CreatePlaylistState,
        // Preference
        PreferenceState,
        // Router
        RouterState,
        // Server
        CurrentServerState,
        // Storage
        StoragesState,
        StorageBackendStaticState,
        CurrentStorageState,
        StoragesRecordState,
        EditStorageState
    ))
}
