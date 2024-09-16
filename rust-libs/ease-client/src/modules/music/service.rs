use std::{collections::HashMap, sync::Arc, time::Duration};

use bytes::Bytes;
use ease_client_shared::{
    backends::{
        music::{Music, MusicId, MusicMeta},
        playlist::{Playlist, PlaylistId},
        storage::{StorageEntry, StorageId},
    },
    uis::{music::PlayMusicEventType, preference::PlayMode, storage::CurrentStorageImportType},
};
use misty_vm::{
    async_task::MistyAsyncTaskTrait,
    client::{AsReadonlyMistyClientHandle, MistyClientHandle},
    resources::MistyResourceHandle,
    services::MistyServiceTrait,
    states::MistyStateTrait,
    MistyAsyncTask, MistyState,
};

use crate::modules::{
    error::{EaseError, EaseResult, EASE_RESULT_NIL},
    playlist::service::{change_current_playlist, get_current_playlist},
    preference::service::{get_playmode, update_playmode},
    storage::service::enter_storages_to_import,
    timer::to_host::TimerService,
};

use super::to_host::MusicPlayerService;

#[derive(Default, Clone, MistyState)]
pub struct CurrentMusicState {
    pub music: Option<Music>,
    pub current_duration: Duration,
    pub current_playlist: Option<Playlist>,
    pub playing: bool,
    pub lyric_line_index: i32,
    pub can_play_next: bool,
    pub can_play_previous: bool,
    pub prev_music: Option<MusicId>,
    pub next_music: Option<MusicId>,
    pub loading: bool,
}

#[derive(Default, Clone, MistyState)]
pub struct TimeToPauseState {
    pub enabled: bool,
    pub expired_time: u64,
    pub left: u64,
}

struct ChangeMusicInfo {
    can_play_next: bool,
    next_index: usize,
    can_play_prev: bool,
    prev_index: usize,
    prev_music: Option<MusicId>,
    next_music: Option<MusicId>,
    music_metas: Vec<MusicMeta>,
}

fn is_current_music_match(client: MistyClientHandle, id: MusicId) -> bool {
    let curr_music_id =
        CurrentMusicState::map(client, |state| state.music.as_ref().map(|m: &Music| m.id()));
    return curr_music_id.is_some() && curr_music_id.unwrap() == id;
}

pub fn get_current_music_id(client: MistyClientHandle) -> Option<MusicId> {
    CurrentMusicState::map(client, |state| state.music.as_ref().map(|m| m.id()))
}

fn compute_change_music_info(
    client: MistyClientHandle,
    state: &CurrentMusicState,
    playmode: PlayMode,
) -> EaseResult<ChangeMusicInfo> {
    let current_music_id = state
        .music
        .as_ref()
        .map(|m| m.id())
        .ok_or(EaseError::CurrentMusicNone)?;

    let music_metas = state.current_playlist.unwrap().musics;
    let index = music_metas
        .iter()
        .enumerate()
        .find(|(_, m)| m.music_id() == current_music_id)
        .map(|(i, _)| i)
        .unwrap();
    let loop_enabled = playmode == PlayMode::SingleLoop || playmode == PlayMode::ListLoop;
    let can_play_prev = loop_enabled || index > 0;
    let prev_index = if index == 0 {
        music_metas.len() - 1
    } else {
        index - 1
    };
    let prev_music = if can_play_prev {
        Some(music_metas[prev_index].id)
    } else {
        None
    };
    let can_play_next = loop_enabled || index + 1 < music_metas.len();
    let next_index = if index + 1 >= music_metas.len() {
        0
    } else {
        index + 1
    };
    let next_music = if can_play_next {
        Some(music_metas[next_index].id)
    } else {
        None
    };

    Ok(ChangeMusicInfo {
        can_play_next,
        next_index,
        next_music,
        can_play_prev,
        prev_index,
        prev_music,
        music_metas,
    })
}

const EXTS: [&str; 4] = [".mp3", ".wav", ".flac", ".acc"];

fn get_display_name(name: &str) -> String {
    for ext in EXTS.iter() {
        let striped = name.strip_suffix(*ext);
        if striped.is_some() {
            return striped.clone().unwrap().to_string();
        }
    }
    return name.to_string();
}

pub fn update_current_music_playing(client: MistyClientHandle, playing: bool) {
    CurrentMusicState::update(client, |state| {
        state.playing = playing;
    });
}

pub fn resume_music(client: MistyClientHandle) {
    let music = CurrentMusicState::map(client, |state| state.music.clone());
    if music.is_none() {
        return;
    }
    MusicPlayerService::of(client).resume();
}

pub fn pause_music(client: MistyClientHandle) {
    MusicPlayerService::of(client).pause();
}

pub fn stop_music(client: MistyClientHandle) {
    CurrentMusicState::update(client, |state| {
        state.current_playlist = None;
        state.music = None;
    });
    MusicPlayerService::of(client).stop();
}

pub fn seek_music(client: MistyClientHandle, duration: u64) {
    MusicPlayerService::of(client).seek(duration);
}

pub fn update_current_music_position(client: MistyClientHandle, duration: Duration) {
    CurrentMusicState::update(client, |state| {
        state.current_duration = duration;
    });
    update_current_music_lyric_index(client);
}

pub fn update_current_music_total_duration(
    client: MistyClientHandle,
    duration: Duration,
) -> EaseResult<()> {
    let info = CurrentMusicState::map(client, |state| {
        state.music.as_ref().map(|m| (m.id, m.duration))
    });
    if info.is_none() {
        return Ok(());
    }
    let info = info.unwrap();
    let (id, total_duration) = info;
    if let Some(total_duration) = total_duration {
        let total_duration: &Duration = &total_duration;
        if total_duration == &duration {
            return Ok(());
        }
    }

    db_update_music_total_duration(client, id, duration)?;

    CurrentMusicState::update(client, |state| {
        let music = state.music.as_mut().unwrap();
        music.set_duration(Some(MusicDuration::new(duration)));
    });
    update_playlists_state_by_music_duration_change(client, id, Some(duration));
    Ok(())
}

fn play_music_impl(
    client: MistyClientHandle,
    music_id: MusicId,
    playlist_id: PlaylistId,
) -> EaseResult<()> {
    let music = db_load_music(client, music_id.clone())?;
    if music.is_none() {
        return Ok(());
    }
    let music = music.unwrap();

    let (prev_current_music_id, prev_playlist_id) = CurrentMusicState::map(client, |state| {
        (
            state.music.as_ref().map(|m| m.id()),
            state.current_playlist.clone(),
        )
    });
    if prev_current_music_id.is_some()
        && prev_current_music_id.as_ref().unwrap() == &music_id
        && prev_playlist_id == Some(playlist_id)
    {
        resume_music(client);
        return Ok(());
    }

    CurrentMusicState::update(client, |state| {
        state.music = Some(music.clone());
        state.current_playlist = Some(playlist_id);
    });
    let next_state = CurrentMusicState::map(client, |state| state.clone());
    let playmode = get_playmode(client);
    let ChangeMusicInfo {
        can_play_next: can_change_next,
        can_play_prev: can_change_prev,
        prev_music,
        next_music,
        ..
    } = compute_change_music_info(client, &next_state, playmode)?;
    CurrentMusicState::update(client, |state| {
        state.can_play_next = can_change_next;
        state.can_play_previous = can_change_prev;
        state.prev_music = prev_music;
        state.next_music = next_music;
    });
    schedule_download_current_music_picture(client)?;
    clear_update_current_music_lyric(client);
    schedule_update_current_music_lyric(client);
    MusicPlayerService::of(client).set_music_url(get_serve_music_url(client, &music));
    resume_music(client);
    Ok(())
}

pub fn play_music(client: MistyClientHandle, music_id: MusicId) -> EaseResult<()> {
    let current_playlist_id = get_current_playlist(client);

    if let Some(playlist_id) = current_playlist_id {
        play_music_impl(client, music_id, playlist_id)?;
    }
    Ok(())
}

fn replay_current_music(client: MistyClientHandle) -> EaseResult<()> {
    let state = CurrentMusicState::map(client, |v| v.clone());
    let current_music_id = state.music.as_ref().map(|m| m.id()).unwrap();
    seek_music(client, 0);
    play_music_impl(client, current_music_id, state.current_playlist.unwrap())?;
    return EASE_RESULT_NIL;
}

fn can_play_next(client: MistyClientHandle) -> EaseResult<bool> {
    let state = CurrentMusicState::map(client, |state| state.clone());
    let playmode = get_playmode(client);
    let ChangeMusicInfo { can_play_next, .. } =
        compute_change_music_info(client, &state, playmode)?;
    return Ok(can_play_next);
}

pub fn play_next_music(client: MistyClientHandle) -> EaseResult<()> {
    let state = CurrentMusicState::map(client, |state| state.clone());
    let playmode = get_playmode(client);
    let ChangeMusicInfo {
        can_play_next,
        next_index,
        music_metas,
        ..
    } = compute_change_music_info(client, &state, playmode)?;
    if !can_play_next {
        return EASE_RESULT_NIL;
    }

    let next_music = &music_metas[next_index];
    play_music_impl(
        client,
        next_music.music_id(),
        state.current_playlist.unwrap(),
    )?;
    return EASE_RESULT_NIL;
}

pub fn play_previous_music(client: MistyClientHandle) -> EaseResult<()> {
    let state = CurrentMusicState::map(client, |state| state.clone());
    let playmode = get_playmode(client);
    let ChangeMusicInfo {
        can_play_prev,
        prev_index,
        music_metas,
        ..
    } = compute_change_music_info(client, &state, playmode)?;
    if !can_play_prev {
        return EASE_RESULT_NIL;
    }

    let next_music = &music_metas[prev_index];
    play_music_impl(
        client,
        next_music.music_id(),
        state.current_playlist.unwrap(),
    )?;
    return EASE_RESULT_NIL;
}

fn handle_music_complete(client: MistyClientHandle) -> EaseResult<()> {
    let play_mode = get_playmode(client);

    match play_mode {
        PlayMode::List => {
            if can_play_next(client)? {
                play_next_music(client)?;
            } else {
                pause_music(client);
                seek_music(client, 0);
            }
        }
        PlayMode::ListLoop => {
            play_next_music(client)?;
        }
        PlayMode::SingleLoop => {
            replay_current_music(client)?;
        }
        PlayMode::Single => {
            pause_music(client);
            seek_music(client, 0);
        }
    }
    Ok(())
}

pub(super) fn handle_play_music_event_for_player_internal(
    client: MistyClientHandle,
    arg: PlayMusicEventType,
) -> EaseResult<()> {
    tracing::info!("play music event {:?}", arg);
    match arg {
        PlayMusicEventType::Complete => handle_music_complete(client)?,
        PlayMusicEventType::Loading => handle_music_source_loading(client)?,
        PlayMusicEventType::Loaded => handle_music_source_loaded(client)?,
    }
    Ok(())
}

fn handle_music_source_loading(client: MistyClientHandle) -> EaseResult<()> {
    CurrentMusicState::update(client, |state| {
        state.loading = true;
    });
    Ok(())
}

fn handle_music_source_loaded(client: MistyClientHandle) -> EaseResult<()> {
    CurrentMusicState::update(client, |state| {
        state.loading = false;
    });
    Ok(())
}

pub(super) fn update_music_play_mode_to_next(client: MistyClientHandle) -> EaseResult<()> {
    let current_playmode: PlayMode = get_playmode(client);
    let next_playmode = match current_playmode {
        PlayMode::Single => PlayMode::SingleLoop,
        PlayMode::SingleLoop => PlayMode::List,
        PlayMode::List => PlayMode::ListLoop,
        PlayMode::ListLoop => PlayMode::Single,
    };
    update_playmode(client, next_playmode)?;

    let state = CurrentMusicState::map(client, |s| s.clone());

    let res = compute_change_music_info(client, &state, next_playmode);
    if let Err(EaseError::CurrentMusicNone) = &res {
        return Ok(());
    }

    let ChangeMusicInfo {
        can_play_next,
        can_play_prev,
        ..
    } = res?;

    CurrentMusicState::update(client, |state| {
        state.can_play_previous = can_play_prev;
        state.can_play_next = can_play_next;
    });

    Ok(())
}

pub fn clear_current_music_state_if_invalid(client: MistyClientHandle) {
    let (music_id, rel_playlist) = CurrentMusicState::map(client, |state| {
        let music_id = state.music.as_ref().map(|m| m.id());
        let playlist = state.current_playlist.clone();

        (music_id, playlist)
    });
    let valid = if music_id.is_none() || rel_playlist.is_none() {
        false
    } else {
        let rel_playlist = rel_playlist.unwrap();
        let music_id = music_id.unwrap();
        rel_playlist.musics.iter().any(|m| m.id == music_id)
    };
    if !valid {
        stop_music(client);
    }
}

#[derive(Debug, MistyAsyncTask)]
struct TimeToPauseAsyncTask;

pub fn remove_time_to_pause(client: MistyClientHandle) {
    TimeToPauseAsyncTask::cancel_all(client);
    TimeToPauseState::update(client, |state| {
        state.enabled = false;
    });
}

pub fn update_time_to_pause(client: MistyClientHandle, duration: u64) {
    let start_time = TimerService::of(client).get_current_time_ms();
    TimeToPauseState::update(client, |state| {
        state.enabled = true;
        state.expired_time = start_time as u64 + duration;
    });

    #[allow(unreachable_code)]
    TimeToPauseAsyncTask::spawn_once(client, |ctx| async move {
        loop {
            ctx.schedule(|client| {
                let state = TimeToPauseState::map(client, |v| v.clone());
                let curr_time = TimerService::of(client).get_current_time_ms() as u64;
                let left = if state.expired_time > curr_time {
                    state.expired_time - curr_time
                } else {
                    0
                };

                TimeToPauseState::update(client, |state| {
                    state.left = left;
                });
                if left == 0 {
                    remove_time_to_pause(client);
                    pause_music(client);
                }
                return EASE_RESULT_NIL;
            });

            TimerService::of_async(&ctx)
                .wait(Duration::from_secs(1))
                .await;
        }
        return EASE_RESULT_NIL;
    });
}

pub(super) fn prepare_import_lyrics_to_current_music(client: MistyClientHandle) -> EaseResult<()> {
    enter_storages_to_import(client, CurrentStorageImportType::CurrentMusicLyrics)?;
    Ok(())
}

pub fn import_selected_lyric_in_music(
    client: MistyClientHandle,
    id: MusicId,
    storage_id: StorageId,
    entry: StorageEntry,
) -> EaseResult<()> {
    let path = entry.path;
    db_update_music_lyric(client, id, storage_id, path.clone())?;
    if is_current_music_match(client, id) {
        CurrentMusicState::update(client, |state| {
            if let Some(music) = state.music.as_mut() {
                music.set_lyric_entry(Some((storage_id, path)));
            }
        });
        schedule_update_current_music_lyric(client);
    }
    Ok(())
}

pub(super) fn remove_current_music_lyric(client: MistyClientHandle) -> EaseResult<()> {
    let id = get_current_music_id(client);
    if let Some(id) = id {
        db_remove_music_lyric(client, id)?;
        CurrentMusicAssetState::update(client, |state| {
            state.lyric_load_state = LyricLoadState::Missing;
            state.lyric = None;
        });
    }
    Ok(())
}

#[derive(Debug, MistyAsyncTask)]
struct UpdateCurrentMusicLyricAsyncTask;

fn clear_update_current_music_lyric(client: MistyClientHandle) {
    CurrentMusicAssetState::update(client, |state| {
        state.lyric = None;
        state.lyric_load_state = LyricLoadState::Missing;
    });
    CurrentMusicState::update(client, |state| {
        state.lyric_line_index = -1;
    });
}

fn update_current_music_lyric_index(client: MistyClientHandle) {
    let music = CurrentMusicState::map(client, |state| state.music.clone());
    if music.is_none() {
        return;
    }
    let lyric = CurrentMusicAssetState::map(client, |state| state.lyric.clone());
    if lyric.is_none() {
        return;
    }
    let lyric = lyric.unwrap();
    let duration = CurrentMusicState::map(client, |state| state.current_duration.clone());

    let i = lyric
        .lines
        .binary_search_by(|(line_duration, _)| line_duration.cmp(&duration))
        .map(|i| i as i32)
        .unwrap_or_else(|i| i as i32 - 1);
    CurrentMusicState::update(client, |state: &mut CurrentMusicState| {
        state.lyric_line_index = i;
    });
}

pub(super) fn change_to_current_music_playlist(client: MistyClientHandle) -> EaseResult<()> {
    let playlist_id = CurrentMusicState::map(client, |state| state.current_playlist);
    if playlist_id.is_none() {
        return Ok(());
    }
    change_current_playlist(client, playlist_id.unwrap());
    Ok(())
}
