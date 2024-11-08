use misty_vm::misty_to_host;

#[derive(uniffi::Enum)]
pub enum RoutesKey {
    Home,
    AddDevices,
    Playlist,
    ImportMusics,
    MusicPlayer,
}

#[uniffi::export(with_foreign)]
pub trait IRouterService: Send + Sync + 'static {
    fn naviagate(&self, key: RoutesKey);
    fn pop(&self);
}
misty_to_host!(RouterService, IRouterService);
