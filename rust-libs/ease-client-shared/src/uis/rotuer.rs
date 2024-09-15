use std::default;

use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum RootRouteSubKey {
    #[default]
    Playlist,
    Dashboard,
    Setting,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VRootSubKeyState {
    pub subkey: RootRouteSubKey,
}
