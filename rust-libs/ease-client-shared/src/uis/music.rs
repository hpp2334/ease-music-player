use serde::Serialize;

use crate::backends::music::MusicId;

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

#[derive(Debug, uniffi::Record)]
pub struct ArgSeekMusic {
    pub duration: u64,
}
