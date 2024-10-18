use misty_vm::misty_to_host;

#[uniffi::export(with_foreign)]
pub trait IMusicPlayerService: Send + Sync + 'static {
    fn resume(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, arg: u64);
    fn set_music_url(&self, url: String);
}
misty_to_host!(MusicPlayerService, IMusicPlayerService);
