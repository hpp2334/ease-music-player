use ease_client_shared::backends::music::{Music, MusicId};
use misty_vm::misty_to_host;

#[derive(Debug, uniffi::Record)]
pub struct MusicToPlay {
    pub id: MusicId,
    pub title: String,
    pub url: String,
    pub cover_url: String,
}

#[uniffi::export(with_foreign)]
pub trait IMusicPlayerService: Send + Sync + 'static {
    fn resume(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, arg: u64);
    fn set_music_url(&self, item: MusicToPlay);
    fn get_current_duration_s(&self) -> u64;
    fn request_total_duration(&self, id: MusicId, url: String);
}
misty_to_host!(MusicPlayerService, IMusicPlayerService);
