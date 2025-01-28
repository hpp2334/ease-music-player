use ease_client_shared::backends::{playlist::PlaylistId, storage::StorageId};
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum RootRouteSubKey {
    #[default]
    Playlist,
    Dashboard,
    Setting,
}

#[derive(Default, Clone)]
pub struct MainState {
    pub subkey: RootRouteSubKey,
    pub vs_loaded: bool,
    pub visible_count: i32,
}

#[derive(Default, Clone)]
pub(crate) enum RightMenuValue {
    #[default]
    None,
    Storage(StorageId)
}

#[derive(Default, Clone)]
pub(crate) struct RightMenuState {
    pub(crate) visible: bool,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) value: RightMenuValue,
}
