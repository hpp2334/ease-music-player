use serde::Serialize;

use crate::MusicId;

use super::preference::PlayMode;

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum PlayMusicEventType {
    Complete,
    Loading,
    Loaded,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum LyricLoadState {
    Loading,
    #[default]
    Missing,
    Failed,
    Loaded,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, uniffi::Record)]
pub struct VLyricLine {
    pub time: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VCurrentMusicLyricState {
    pub load_state: LyricLoadState,
    pub lyric_lines: Vec<VLyricLine>,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VTimeToPauseState {
    pub enabled: bool,
    pub left_hour: u64,
    pub left_minute: u64,
}
