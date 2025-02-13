use ease_client_shared::backends::playlist::PlaylistId;
use misty_vm::misty_to_host;

#[derive(uniffi::Enum)]
pub enum AndroidRoutesKey {
    Home,
    AddDevices,
    Playlist,
    ImportMusics,
    MusicPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopRoutesKey {
    Home,
    Setting,
    Playlist,
}

pub trait IRouterService: 'static {
    fn navigate(&self, key: AndroidRoutesKey) {}
    fn navigate_desktop(&self, key: DesktopRoutesKey) {}
    fn pop(&self);
}
misty_to_host!(RouterService, IRouterService);
