use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct VRootSubKeyState {
    pub subkey: RootRouteSubKey,
}
