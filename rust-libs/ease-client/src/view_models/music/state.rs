use std::{rc::Rc, time::Duration};

use ease_client_shared::{
    backends::{
        music::{MusicAbstract, MusicId, MusicLyric},
        playlist::PlaylistId,
    },
    uis::preference::PlayMode,
};

#[derive(Clone)]
pub struct QueueMusic {
    pub id: MusicId,
    pub playlist_id: PlaylistId,
    pub queue: Rc<Vec<MusicAbstract>>,
    pub index: usize,
}

#[derive(Default, Clone)]
pub struct CurrentMusicState {
    pub music: Option<QueueMusic>,
    pub current_duration: Duration,
    pub playing: bool,
    pub play_mode: PlayMode,
    pub lyric: Option<MusicLyric>,
    pub lyric_line_index: i32,
    pub loading: bool,
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
        self.music.as_ref().map(|v| v.id)
    }

    pub fn can_play_next(&self) -> bool {
        if let Some(QueueMusic { queue, index, .. }) = self.music.as_ref() {
            match self.play_mode {
                PlayMode::SingleLoop | PlayMode::ListLoop => true,
                _ => index + 1 < queue.len(),
            }
        } else {
            false
        }
    }

    pub fn can_play_previous(&self) -> bool {
        if let Some(QueueMusic { index, .. }) = self.music.as_ref() {
            match self.play_mode {
                PlayMode::SingleLoop | PlayMode::ListLoop => true,
                _ => *index > 0,
            }
        } else {
            false
        }
    }

    pub fn cover(&self) -> String {
        if let Some(QueueMusic { queue, index, .. }) = self.music.as_ref() {
            if let Some(music) = queue.get(*index) {
                return music.cover_url.clone();
            }
        }
        String::new()
    }

    pub fn prev_cover(&self) -> String {
        if let Some(QueueMusic { queue, index, .. }) = self.music.as_ref() {
            if self.can_play_previous() {
                let prev_index = if *index == 0 {
                    queue.len() - 1
                } else {
                    *index - 1
                };
                if let Some(music) = queue.get(prev_index) {
                    return music.cover_url.clone();
                }
            }
        }
        String::new()
    }

    pub fn next_cover(&self) -> String {
        if let Some(QueueMusic { queue, index, .. }) = self.music.as_ref() {
            if self.can_play_next() {
                let next_index = if *index + 1 >= queue.len() {
                    0
                } else {
                    *index + 1
                };
                if let Some(music) = queue.get(next_index) {
                    return music.cover_url.clone();
                }
            }
        }
        String::new()
    }
}
