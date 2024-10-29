use ease_client_shared::{
    backends::{music::{LyricLoadState, MusicId}, music_duration::MusicDuration},
    uis::preference::PlayMode,
};
use serde::Serialize;

use crate::{
    utils::common::get_display_duration,
    view_models::music::{state::{CurrentMusicState, TimeToPauseState}},
};

use super::models::RootViewModelState;

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
    pub previous_cover: String,
    pub next_cover: String,
    pub cover: String,
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
    pub lyric_lines: Vec<VLyricLine>,
    pub load_state: LyricLoadState
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VTimeToPauseState {
    pub enabled: bool,
    pub left_hour: u64,
    pub left_minute: u64,
}

pub(crate) fn current_music_vs(state: &CurrentMusicState, root: &mut RootViewModelState) {
    let id = {
        let id = state.id;
        if let Some(id) = id {
            id
        } else {
            root.current_music = Some(Default::default());
            return;
        }
    };
    let abstr = &state.playlist_musics[state.index_musics];

    let title = abstr.meta.title.clone();
    let total_duration = abstr.meta.duration.clone();
    let current_duration = MusicDuration::new(state.current_duration);

    let view_model_state = VCurrentMusicState {
        id: Some(id),
        title,
        current_duration: get_display_duration(&Some(current_duration)),
        total_duration: get_display_duration(&total_duration),
        current_duration_ms: current_duration.as_millis() as u64,
        total_duration_ms: total_duration.map_or(i32::MAX as u64, |d| d.as_millis() as u64),
        can_change_position: total_duration.is_some(),
        can_play_next: state.can_play_next(),
        can_play_previous: state.can_play_previous(),
        previous_cover: state.prev_cover(),
        next_cover: state.next_cover(),
        cover: state.cover(),
        playing: state.playing,
        play_mode: state.play_mode,
        lyric_index: state.lyric_line_index,
        loading: state.loading,
    };

    root.current_music = Some(view_model_state);
}

pub(crate) fn time_to_pause_vs(state: &TimeToPauseState, root: &mut RootViewModelState) {
    let state = state.clone();
    let enabled = state.enabled;
    let left_hour = (state.left.as_millis() / 3600_000) as u64;
    let left_minute = (state.left.as_millis() / 60_000 % 60) as u64;

    root.time_to_pause = Some(VTimeToPauseState {
        enabled,
        left_hour,
        left_minute,
    });
}

pub(crate) fn current_music_lyric_vs(state: &CurrentMusicState, root: &mut RootViewModelState) {
    let lyric_lines = state.lyric.as_ref().map(|lyrics| {
        let lines: Vec<_> = lyrics
            .data
            .lines
            .clone()
            .into_iter()
            .map(|(duration, line)| (duration.as_millis() as u64, line))
            .collect();
        lines
    });
    let lyric_lines = lyric_lines.unwrap_or_default();

    root.current_music_lyric = Some(VCurrentMusicLyricState {
        lyric_lines: lyric_lines
            .into_iter()
            .map(|(time, text)| VLyricLine {
                time: time as u32,
                text,
            })
            .collect(),
        load_state: if state.lyric.is_some() {
            state.lyric.as_ref().unwrap().loaded_state
        } else {
            LyricLoadState::Missing
        }
    });
}
