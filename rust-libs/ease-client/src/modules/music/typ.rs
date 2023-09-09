use std::time::Duration;

use bytes::Bytes;

use serde::Serialize;

use crate::{define_id, modules::preference::PlayMode};

define_id!(MusicId);

pub struct MusicMeta {
    pub buf: Option<Bytes>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
pub enum PlayMusicEventType {
    Complete,
    Loading,
    Loaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LyricLoadState {
    Loading,
    Missing,
    Failed,
    Loaded,
}

impl Default for LyricLoadState {
    fn default() -> Self {
        Self::Missing
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VCurrentMusicState {
    pub id: Option<MusicId>,
    pub title: String,
    pub current_duration: String,
    pub total_duration: String,
    pub current_duration_ms: u64,
    pub total_duration_ms: u64,
    pub can_change_position: bool,
    pub can_play_next: bool,
    pub can_play_previous: bool,
    pub previous_cover: u64,
    pub next_cover: u64,
    pub cover: u64,
    pub play_mode: PlayMode,
    pub playing: bool,
    pub lyric_index: i32,
    pub loading: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VCurrentMusicLyricState {
    pub load_state: LyricLoadState,
    pub lyric_lines: Vec<(u64, String)>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VTimeToPauseState {
    pub enabled: bool,
    pub left_hour: u64,
    pub left_minute: u64,
}
