use std::{
    sync::{atomic::AtomicBool, RwLock},
    time::Duration,
};

use ease_client_shared::backends::{
    connector::ConnectorAction,
    music::{
        LyricLoadState, Music, MusicAbstract, MusicId, MusicLyric, MusicMeta, TimeToPauseInfo,
    },
    music_duration::MusicDuration,
    storage::{DataSourceKey, StorageEntryLoc},
};
use misty_async::Task;

use crate::{ctx::BackendContext, error::BResult, models::music::MusicModel};

use super::{
    lyrics::parse_lrc, player::player_refresh_current, playlist::notify_all_playlist_abstracts,
    storage::load_storage_entry_data,
};

#[derive(Debug, Default)]
pub struct TimeToPauseState {
    enabled: AtomicBool,
    expired: RwLock<Duration>,
    task: RwLock<Option<Task<()>>>,
}

async fn load_lyric(
    cx: &BackendContext,
    loc: Option<StorageEntryLoc>,
    is_fallback: bool,
) -> Option<MusicLyric> {
    let loc = match loc {
        Some(loc) => loc,
        None => {
            return None;
        }
    };
    let data = load_storage_entry_data(&cx, &loc).await;
    if let Err(e) = &data {
        tracing::error!("fail to load entry {:?}: {}", loc, e);
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: if is_fallback {
                LyricLoadState::Missing
            } else {
                LyricLoadState::Failed
            },
        });
    }
    let data = data.unwrap();
    if data.is_none() {
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: if is_fallback {
                LyricLoadState::Missing
            } else {
                LyricLoadState::Failed
            },
        });
    }
    let data = data.unwrap();
    let data = String::from_utf8_lossy(&data).to_string();
    let lyric = parse_lrc(data);
    if lyric.is_err() {
        let e = lyric.unwrap_err();
        tracing::error!("fail to parse lyric: {}", e);
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: LyricLoadState::Failed,
        });
    }
    let lyric = lyric.unwrap();

    Some(MusicLyric {
        loc,
        data: lyric,
        loaded_state: LyricLoadState::Loaded,
    })
}

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
    }
}

pub(crate) fn build_music_abstract(_cx: &BackendContext, model: MusicModel) -> MusicAbstract {
    let cover = if model.cover.is_some() {
        Some(DataSourceKey::Cover { id: model.id })
    } else {
        Default::default()
    };

    MusicAbstract {
        cover,
        meta: build_music_meta(model),
    }
}

pub fn get_music_storage_entry_loc(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<StorageEntryLoc>> {
    let m = cx.database_server().load_music(id)?;
    if m.is_none() {
        return Ok(None);
    }
    let m = m.unwrap();
    let m = m.loc;
    Ok(Some(m))
}

pub fn get_music_cover_bytes(cx: &BackendContext, id: MusicId) -> BResult<Vec<u8>> {
    let m = cx.database_server().load_music(id)?.unwrap();
    if let Some(id) = m.cover {
        cx.database_server().blob().read(id)
    } else {
        Ok(Default::default())
    }
}

pub(crate) struct ArgUpdateMusicDuration {
    pub id: MusicId,
    pub duration: MusicDuration,
}
pub(crate) async fn update_music_duration(
    cx: &BackendContext,
    arg: ArgUpdateMusicDuration,
) -> BResult<()> {
    cx.database_server()
        .update_music_total_duration(arg.id, arg.duration)?;
    player_refresh_current(cx).await?;
    cx.notify(ConnectorAction::MusicTotalDurationChanged(arg.id));
    notify_all_playlist_abstracts(&cx).await?;
    Ok(())
}

pub(crate) struct ArgUpdateMusicCover {
    pub id: MusicId,
    pub cover: Vec<u8>,
}
pub(crate) async fn update_music_cover(
    cx: &BackendContext,
    arg: ArgUpdateMusicCover,
) -> BResult<()> {
    cx.database_server()
        .update_music_cover(arg.id, arg.cover.clone())?;
    player_refresh_current(cx).await?;
    cx.notify(ConnectorAction::MusicCoverChanged(arg.id));
    notify_all_playlist_abstracts(&cx).await?;
    Ok(())
}

pub(crate) async fn get_music(cx: &BackendContext, id: MusicId) -> BResult<Option<Music>> {
    let model = cx.database_server().load_music(id)?;
    if model.is_none() {
        return Ok(None);
    }

    let model = model.unwrap();
    let meta = build_music_meta(model.clone());
    let loc = model.loc;
    let mut lyric_loc = model.lyric;
    let using_fallback = lyric_loc.is_none() && model.lyric_default;
    if using_fallback {
        lyric_loc = Some(StorageEntryLoc {
            path: {
                let mut path = loc.path.clone();
                let new_extension = ".lrc";
                if let Some(pos) = path.rfind('.') {
                    path.truncate(pos);
                }
                path.push_str(new_extension);
                path
            },
            storage_id: loc.storage_id,
        });
    }

    let lyric: Option<MusicLyric> = load_lyric(&cx, lyric_loc, using_fallback).await;
    let cover = if model.cover.is_none() {
        Default::default()
    } else {
        Some(DataSourceKey::Cover { id: model.id })
    };

    let music: Music = Music {
        meta,
        loc,
        cover,
        lyric,
    };
    Ok(Some(music))
}

pub(crate) async fn notify_music(cx: &BackendContext, id: MusicId) -> BResult<()> {
    let music = get_music(cx, id).await?;
    if let Some(music) = music {
        cx.notify(ConnectorAction::Music(music));
    }
    Ok(())
}

pub(crate) fn enable_time_to_pause(cx: &BackendContext, delay: Duration) {
    let state = cx.time_to_pause_state().clone();
    state.task.write().unwrap().take();
    state
        .enabled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    *state.expired.write().unwrap() = cx.async_runtime().get_time() + delay;
    let task = {
        let rt = cx.async_runtime().clone();
        let cx = cx.weak();
        rt.clone().spawn(async move {
            rt.sleep(delay).await;
            state
                .enabled
                .store(false, std::sync::atomic::Ordering::Relaxed);
            if let Some(cx) = cx.upgrade() {
                sync_notify_time_to_pause(&cx);
            }
            {
                let cx = cx.clone();
                rt.clone()
                    .spawn_on_main(async move {
                        if let Some(cx) = cx.upgrade() {
                            cx.player_delegate().pause();
                        }
                    })
                    .await
            }
        })
    };
    {
        let mut w = cx.time_to_pause_state().task.write().unwrap();
        *w = Some(task);
    }
    sync_notify_time_to_pause(cx);
}

pub(crate) fn disable_time_to_pause(cx: &BackendContext) {
    cx.time_to_pause_state().task.write().unwrap().take();
    cx.time_to_pause_state()
        .enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    sync_notify_time_to_pause(cx);
}

pub(crate) fn sync_notify_time_to_pause(cx: &BackendContext) {
    let state = cx.time_to_pause_state().clone();
    let enabled = state.enabled.load(std::sync::atomic::Ordering::Relaxed);
    let expired = state.expired.read().unwrap().clone();
    let current_time = cx.async_runtime().get_time();
    let left = if current_time < expired {
        expired - current_time
    } else {
        Duration::ZERO
    };

    cx.notify(ConnectorAction::TimeToPause(TimeToPauseInfo {
        enabled,
        expired,
        left,
    }));
}

pub(crate) async fn notify_time_to_pause(cx: &BackendContext) -> BResult<()> {
    sync_notify_time_to_pause(cx);
    Ok(())
}
