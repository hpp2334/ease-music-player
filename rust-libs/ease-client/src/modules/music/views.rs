use misty_vm::resources::MistyResourceId;

use crate::{
    core_views::RootViewModelState, modules::PreferenceState, utils::get_display_duration,
};

use super::{
    repository::MusicDuration,
    service::{CurrentMusicAssetState, CurrentMusicState, TimeToPauseState},
    typ::*,
};

pub fn current_music_view_model(
    (state, asset_state, preference): (
        &CurrentMusicState,
        &CurrentMusicAssetState,
        &PreferenceState,
    ),
    root: &mut RootViewModelState,
) {
    let music = state.music.as_ref();

    let id = music.map(|v| v.id());
    let title = music.map_or(Default::default(), |v| v.title().to_string());
    let total_duration = music.map_or(Default::default(), |v| v.duration().clone());
    let current_duration = MusicDuration::new(state.current_duration);

    let view_model_state = VCurrentMusicState {
        id,
        title,
        current_duration: get_display_duration(&Some(current_duration)),
        total_duration: get_display_duration(&total_duration),
        current_duration_ms: current_duration.as_millis() as u64,
        total_duration_ms: total_duration.map_or(i32::MAX as u64, |d| d.as_millis() as u64),
        can_change_position: total_duration.is_some(),
        can_play_next: state.can_play_next,
        can_play_previous: state.can_play_previous,
        previous_cover: *state
            .prev_cover
            .as_ref()
            .map(|p| p.id())
            .unwrap_or(MistyResourceId::invalid()),
        next_cover: *state
            .next_cover
            .as_ref()
            .map(|p| p.id())
            .unwrap_or(MistyResourceId::invalid()),
        cover: *asset_state
            .cover_buf
            .as_ref()
            .map(|p| p.id())
            .unwrap_or(MistyResourceId::invalid()),
        playing: state.playing,
        play_mode: preference.play_mode.clone(),
        lyric_index: state.lyric_line_index,
        loading: state.loading,
    };

    root.current_music = Some(view_model_state);
}

pub fn time_to_pause_view_model(state: &TimeToPauseState, root: &mut RootViewModelState) {
    let state = state.clone();
    let enabled = state.enabled;
    let left_hour = state.left / 3600_000;
    let left_minute = state.left / 60_000 % 60;

    root.time_to_pause = Some(VTimeToPauseState {
        enabled,
        left_hour,
        left_minute,
    });
}

pub fn current_music_lyric_view_model(
    state: &CurrentMusicAssetState,
    root: &mut RootViewModelState,
) {
    let lyric_lines = state.lyric.as_ref().map(|lyrics| {
        let lines: Vec<_> = lyrics
            .lines
            .clone()
            .into_iter()
            .map(|(duration, line)| (duration.as_millis() as u64, line))
            .collect();
        lines
    });
    let lyric_lines = lyric_lines.unwrap_or_default();

    root.current_music_lyric = Some(VCurrentMusicLyricState {
        load_state: state.lyric_load_state,
        lyric_lines: lyric_lines
            .into_iter()
            .map(|(time, text)| VLyricLine {
                time: time as u32,
                text,
            })
            .collect(),
    });
}
