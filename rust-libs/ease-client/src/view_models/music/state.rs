use std::{cell::RefCell, rc::Rc, time::Duration};

use ease_client_shared::backends::{
    music::{MusicId, MusicLyric},
    player::{PlayMode, PlayerCurrentPlaying},
};
use misty_vm::AsyncTaskId;

#[derive(Default, Clone)]
pub struct CurrentMusicState {
    pub music: Option<PlayerCurrentPlaying>,
    pub current_duration: Duration,
    pub buffer_duration: Duration,
    pub playing: bool,
    pub play_mode: PlayMode,
    pub lyric: Option<MusicLyric>,
    pub lyric_line_index: i32,
    pub loading: bool,
    pub timer_id: Rc<RefCell<Option<AsyncTaskId>>>,
}

#[derive(Default, Clone)]
pub struct TimeToPauseState {
    pub enabled: bool,
    pub expired_time: Duration,
    pub left: Duration,
    pub modal_open: bool,
}

impl CurrentMusicState {
    pub fn id(&self) -> Option<MusicId> {
        self.music.as_ref().map(|v| v.abstr.id())
    }
}
