use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum RootRouteSubKey {
    Playlist,
    Dashboard,
    Setting,
}

impl Default for RootRouteSubKey {
    fn default() -> Self {
        RootRouteSubKey::Playlist
    }
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VRootSubKeyState {
    pub subkey: RootRouteSubKey,
}
