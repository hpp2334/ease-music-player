use misty_vm::misty_service;

pub trait IMusicPlayerService: Send + Sync + 'static {
    fn resume(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, arg: u64);
    fn set_music_url(&self, url: String);
}

misty_service!(MusicPlayerService, IMusicPlayerService);
