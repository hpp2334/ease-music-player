#[uniffi::export(with_foreign)]
pub trait IMusicPlayerService: Send + Sync + 'static {
    fn resume(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, arg: u64);
    fn set_music_url(&self, url: String);
}

pub trait IToastService: Send + Sync + 'static {
    fn error(&self, msg: String);
}
