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
