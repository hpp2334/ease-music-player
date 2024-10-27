
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum RootRouteSubKey {
    #[default]
    Playlist,
    Dashboard,
    Setting,
}

#[derive(Default, Clone)]
pub struct RouterState {
    pub subkey: RootRouteSubKey,
}