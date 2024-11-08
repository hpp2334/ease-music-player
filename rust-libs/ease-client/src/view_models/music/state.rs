use std::time::Duration;

use ease_client_shared::{
    backends::{
        music::{MusicAbstract, MusicId, MusicLyric},
        playlist::PlaylistId,
    },
    uis::preference::PlayMode,
};

#[derive(Default, Clone)]
pub struct CurrentMusicState {
    pub id: Option<MusicId>,
    pub playlist_id: Option<PlaylistId>,
    pub playlist_musics: Vec<MusicAbstract>,
    pub index_musics: usize,
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
    pub fn can_play_next(&self) -> bool {
        match self.play_mode {
            PlayMode::SingleLoop | PlayMode::ListLoop => true,
            _ => self.index_musics + 1 < self.playlist_musics.len(),
        }
    }

    pub fn can_play_previous(&self) -> bool {
        match self.play_mode {
            PlayMode::SingleLoop | PlayMode::ListLoop => true,
            _ => self.index_musics > 0,
        }
    }

    pub fn cover(&self) -> String {
        if let Some(id) = self.id {
            if let Some(music) = self.playlist_musics.iter().find(|&m| m.id() == id) {
                return music.cover_url.clone();
            }
        }
        String::new()
    }

    pub fn prev_cover(&self) -> String {
        if self.can_play_previous() {
            let prev_index = if self.index_musics == 0 {
                self.playlist_musics.len() - 1
            } else {
                self.index_musics - 1
            };
            self.playlist_musics[prev_index].cover_url.clone()
        } else {
            String::new()
        }
    }

    pub fn next_cover(&self) -> String {
        if self.can_play_next() {
            let next_index = if self.index_musics + 1 >= self.playlist_musics.len() {
                0
            } else {
                self.index_musics + 1
            };
            self.playlist_musics[next_index].cover_url.clone()
        } else {
            String::new()
        }
    }
}
