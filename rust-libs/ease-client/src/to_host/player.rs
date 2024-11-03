use ease_client_shared::backends::music::MusicId;
use misty_vm::misty_to_host;

#[uniffi::export(with_foreign)]
pub trait IMusicPlayerService: Send + Sync + 'static {
    fn resume(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, arg: u64);
    fn set_music_url(&self, id: MusicId, url: String);
    fn get_current_duration_s(&self) -> u64;
}
misty_to_host!(MusicPlayerService, IMusicPlayerService);
