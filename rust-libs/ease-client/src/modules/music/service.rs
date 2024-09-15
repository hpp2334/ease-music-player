use std::{collections::HashMap, sync::Arc, time::Duration};

use bytes::Bytes;
use ease_client_shared::{MusicDuration, MusicId, PlaylistId, StorageEntry, StorageId};
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
    playlist::service::{
        change_current_playlist, get_current_playlist_id, get_playlist,
        update_playlists_state_by_music_duration_change,
    },
    preference::service::{get_playmode, update_playmode},
    storage::service::enter_storages_to_import,
    timer::to_host::TimerService,
    CurrentStorageImportType, PlayMode,
};

use super::{to_host::MusicPlayerService, typ::*};

#[derive(Default, Clone, MistyState)]
pub struct CurrentMusicState {
    pub music: Option<Music>,
    pub current_duration: Duration,
    pub current_playlist_id: Option<PlaylistId>,
    pub playing: bool,
    pub lyric_line_index: i32,
    pub can_play_next: bool,
    pub can_play_previous: bool,
    pub prev_id: Option<MusicId>,
    pub next_id: Option<MusicId>,
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
    prev_cover: Option<MistyResourceHandle>,
    next_cover: Option<MistyResourceHandle>,
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

    let ordered_musics = {
        let playlist = get_playlist(client, state.current_playlist_id.clone().unwrap()).unwrap();
        let orederd_musics = playlist.get_ordered_musics();
        orederd_musics
    };
    let index = ordered_musics
        .iter()
        .enumerate()
        .find(|(_, m)| m.music_id() == current_music_id)
        .map(|(i, _)| i)
        .unwrap();
    let loop_enabled = playmode == PlayMode::SingleLoop || playmode == PlayMode::ListLoop;
    let can_play_prev = loop_enabled || index > 0;
    let prev_index = if index == 0 {
        ordered_musics.len() - 1
    } else {
        index - 1
    };
    let prev_cover = if can_play_prev {
        let id = ordered_musics[prev_index].music_id();
        let cover = db_load_music_picture(client, id)?;
        cover.map(|buf| insert_resource_handle(client, id, buf.to_vec()))
    } else {
        None
    };
    let can_play_next = loop_enabled || index + 1 < ordered_musics.len();
    let next_index = if index + 1 >= ordered_musics.len() {
        0
    } else {
        index + 1
    };
    let next_cover = if can_play_next {
        let id = ordered_musics[next_index].music_id();
        let cover = db_load_music_picture(client, id)?;
        cover.map(|buf| insert_resource_handle(client, id, buf.to_vec()))
    } else {
        None
    };

    Ok(ChangeMusicInfo {
        can_play_next,
        next_index,
        next_cover,
        can_play_prev,
        prev_index,
        prev_cover,
        ordered_musics,
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
        state.current_playlist_id = None;
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
        state.music.as_ref().map(|m| (m.id(), m.duration()))
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
            state.current_playlist_id.clone(),
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
        state.current_playlist_id = Some(playlist_id);
    });
    let next_state = CurrentMusicState::map(client, |state| state.clone());
    let playmode = get_playmode(client);
    let ChangeMusicInfo {
        can_play_next: can_change_next,
        can_play_prev: can_change_prev,
        prev_cover,
        next_cover,
        ..
    } = compute_change_music_info(client, &next_state, playmode)?;
    CurrentMusicState::update(client, |state| {
        state.can_play_next = can_change_next;
        state.can_play_previous = can_change_prev;
        state.prev_id = prev_cover;
        state.next_id = next_cover;
    });
    reload_current_music_picture(client)?;
    schedule_download_current_music_picture(client)?;
    clear_update_current_music_lyric(client);
    schedule_update_current_music_lyric(client);
    MusicPlayerService::of(client).set_music_url(get_serve_music_url(client, &music));
    resume_music(client);
    Ok(())
}

pub fn play_music(client: MistyClientHandle, music_id: MusicId) -> EaseResult<()> {
    let current_playlist_id = get_current_playlist_id(client);

    if let Some(playlist_id) = current_playlist_id {
        play_music_impl(client, music_id, playlist_id)?;
    }
    Ok(())
}

fn replay_current_music(client: MistyClientHandle) -> EaseResult<()> {
    let state = CurrentMusicState::map(client, |v| v.clone());
    let current_music_id = state.music.as_ref().map(|m| m.id()).unwrap();
    seek_music(client, 0);
    play_music_impl(client, current_music_id, state.current_playlist_id.unwrap())?;
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
        ordered_musics,
        ..
    } = compute_change_music_info(client, &state, playmode)?;
    if !can_play_next {
        return EASE_RESULT_NIL;
    }

    let next_music = &ordered_musics[next_index];
    play_music_impl(
        client,
        next_music.music_id(),
        state.current_playlist_id.unwrap(),
    )?;
    return EASE_RESULT_NIL;
}

pub fn play_previous_music(client: MistyClientHandle) -> EaseResult<()> {
    let state = CurrentMusicState::map(client, |state| state.clone());
    let playmode = get_playmode(client);
    let ChangeMusicInfo {
        can_play_prev,
        prev_index,
        ordered_musics,
        ..
    } = compute_change_music_info(client, &state, playmode)?;
    if !can_play_prev {
        return EASE_RESULT_NIL;
    }

    let next_music = &ordered_musics[prev_index];
    play_music_impl(
        client,
        next_music.music_id(),
        state.current_playlist_id.unwrap(),
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
    let (music_id, playlist_id) = CurrentMusicState::map(client, |state| {
        let music_id = state.music.as_ref().map(|m| m.id());
        let playlist_id = state.current_playlist_id.clone();

        (music_id, playlist_id)
    });
    let rel_playlist = if playlist_id.is_none() {
        None
    } else {
        get_playlist(client, playlist_id.unwrap())
    };
    let valid = if music_id.is_none() || rel_playlist.is_none() {
        false
    } else {
        let rel_playlist = rel_playlist.unwrap();
        rel_playlist.musics().contains_key(&music_id.unwrap())
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
struct UpdateMusicsMetadataWhenImportingAsyncTask;

pub fn schedule_download_musics_metadata_when_importing(
    client: MistyClientHandle,
    musics: Vec<Music>,
) {
    UpdateMusicsMetadataWhenImportingAsyncTask::spawn_once(client, |ctx| async move {
        for music in musics.into_iter() {
            let id = music.id();
            let metadata = {
                let handle = ctx.handle();
                let handle = handle.handle();
                get_music_metadata(handle, music).await?
            };

            ctx.schedule(move |client| -> EaseResult<()> {
                db_update_music_picture_and_duration(client, id, &metadata)?;

                let duration = metadata.as_ref().map(|m| m.duration).unwrap_or_default();
                let cover = metadata
                    .map(|m| {
                        m.buf
                            .map(|buf| insert_resource_handle(client, id, buf.to_vec()))
                    })
                    .unwrap_or_default();
                update_playlists_state_by_music_duration_change(client, id, duration);
                update_playlists_state_by_music_cover_change(client, id, cover)?;
                Ok(())
            });
        }
        EaseResult::<()>::Ok(())
    });
}

async fn get_music_metadata(
    client: impl AsReadonlyMistyClientHandle<'_>,
    music: Music,
) -> EaseResult<Option<MusicMeta>> {
    let stream_file = load_music_data(client, music.clone()).await?;
    let chunk = if stream_file.url().ends_with(".mp3") {
        stream_file.bytes().await?
    } else {
        stream_file.chunk_small().await?
    };

    let tag = MTag::read_from(chunk);
    if let Err(_) = tag {
        return Ok(None);
    }
    let tag = tag.unwrap();

    let buf = tag.pic().map(|pic| pic.buf);

    let metadata = MusicMeta {
        buf,
        duration: tag.duration(),
    };
    Ok(Some(metadata))
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

fn schedule_update_current_music_lyric(client: MistyClientHandle) {
    let music = CurrentMusicState::map(client, |state| state.music.clone());
    if music.is_none() {
        return;
    }
    let music = music.unwrap();

    CurrentMusicAssetState::update(client, |state| {
        state.lyric_load_state = LyricLoadState::Loading;
    });

    UpdateCurrentMusicLyricAsyncTask::spawn_once(client, move |ctx| async move {
        let mut stream_file: Option<StreamFile> = None;
        let mut lyric_load_state = LyricLoadState::Loading;

        if let Some((lyric_storage_id, lyric_path)) = music.lyric_entry() {
            let data = {
                let handle = ctx.handle();
                let handle = handle.handle();
                load_storage_entry_data(handle, lyric_storage_id, lyric_path.clone()).await
            };

            if let Err(e) = &data {
                tracing::error!("load lyric fail, path: {}, error: {:?}", lyric_path, e);
                if let EaseError::BackendError(e) = e {
                    if e.is_not_found() {
                        lyric_load_state = LyricLoadState::Missing;
                    } else {
                        lyric_load_state = LyricLoadState::Failed;
                    }
                } else {
                    lyric_load_state = LyricLoadState::Failed;
                }
            } else {
                stream_file = Some(data.unwrap());
            }
        } else {
            lyric_load_state = LyricLoadState::Missing;
        }

        if stream_file.is_none() {
            ctx.schedule(move |client| {
                CurrentMusicAssetState::update(client, |state| {
                    state.lyric_load_state = lyric_load_state;
                });
                return EASE_RESULT_NIL;
            });
            return EASE_RESULT_NIL;
        }

        let f = || async {
            let data = stream_file.unwrap();
            let lrc_data = data.bytes().await?;
            let lrc_data = {
                let lrc_data = String::from_utf8(lrc_data.to_vec())
                    .map_err(|_| EaseError::OtherError("lyric to utf-8 error".to_string()))?;
                let lrc_data = parse_lrc(lrc_data)?;
                lrc_data
            };

            ctx.schedule(|client| {
                CurrentMusicAssetState::update(client, |state: &mut CurrentMusicAssetState| {
                    state.lyric_load_state = LyricLoadState::Loaded;
                    state.lyric = Some(Arc::new(lrc_data));
                });
                return EASE_RESULT_NIL;
            });
            EASE_RESULT_NIL
        };
        let res = f().await;
        if res.is_err() {
            tracing::error!("update lyric fail: {}", res.unwrap_err());
            ctx.schedule(|client| {
                CurrentMusicAssetState::update(client, |state| {
                    state.lyric_load_state = LyricLoadState::Failed;
                });
                return EASE_RESULT_NIL;
            });
        }
        EASE_RESULT_NIL
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
    let playlist_id = CurrentMusicState::map(client, |state| state.current_playlist_id);
    if playlist_id.is_none() {
        return Ok(());
    }
    change_current_playlist(client, playlist_id.unwrap());
    Ok(())
}
