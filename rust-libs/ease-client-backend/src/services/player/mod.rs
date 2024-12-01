use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use ease_client_shared::backends::{
    connector::ConnectorAction,
    music::{MusicAbstract, MusicId},
    music_duration::MusicDuration,
    player::{
        ConnectorPlayerAction, PlayMode, PlayerCurrentPlaying, PlayerDelegateEvent, PlayerDurations,
    },
    playlist::PlaylistId,
    storage::DataSourceKey,
};

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
};

use super::{
    music::{
        update_music_cover, update_music_duration, ArgUpdateMusicCover, ArgUpdateMusicDuration,
    },
    playlist::get_playlist,
};

#[derive(Debug, uniffi::Record)]
pub struct MusicToPlay {
    pub id: MusicId,
    pub url: String,
    pub title: String,
    pub has_cover: bool,
}

pub trait IPlayerDelegate: Send + Sync + 'static {
    fn is_playing(&self) -> bool;
    fn resume(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, arg: u64);
    fn set_music_url(&self, item: MusicToPlay);
    fn get_durations(&self) -> PlayerDurations;
    fn request_total_duration(&self, id: MusicId, url: String);
}

#[derive(Clone)]
pub struct PlayerMedia {
    pub id: MusicId,
    pub playlist_id: PlaylistId,
    pub queue: Arc<Vec<MusicAbstract>>,
    pub index: usize,
}

#[derive(Default)]
pub struct PlayerState {
    pub current: RwLock<Option<PlayerMedia>>,
    pub playmode: RwLock<PlayMode>,
}

impl PlayerState {
    #[allow(dead_code)]
    pub fn id(&self) -> Option<MusicId> {
        self.current.read().unwrap().as_ref().map(|v| v.id)
    }

    pub fn can_play_next(&self) -> bool {
        if let Some(PlayerMedia { queue, index, .. }) = self.current.read().unwrap().as_ref() {
            match *self.playmode.read().unwrap() {
                PlayMode::SingleLoop | PlayMode::ListLoop => true,
                _ => index + 1 < queue.len(),
            }
        } else {
            false
        }
    }

    pub fn can_play_previous(&self) -> bool {
        if let Some(PlayerMedia { index, .. }) = self.current.read().unwrap().as_ref() {
            match *self.playmode.read().unwrap() {
                PlayMode::SingleLoop | PlayMode::ListLoop => true,
                _ => *index > 0,
            }
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn cover(&self) -> Option<DataSourceKey> {
        if let Some(PlayerMedia { queue, index, .. }) = self.current.read().unwrap().as_ref() {
            if let Some(music) = queue.get(*index) {
                return music.cover.clone();
            }
        }
        Default::default()
    }

    pub fn prev_music(&self) -> Option<MusicAbstract> {
        if let Some(PlayerMedia { queue, index, .. }) = self.current.read().unwrap().as_ref() {
            if self.can_play_previous() {
                let prev_index = if *index == 0 {
                    queue.len() - 1
                } else {
                    *index - 1
                };
                if let Some(music) = queue.get(prev_index) {
                    return Some(music.clone());
                }
            }
        }
        Default::default()
    }

    pub fn next_music(&self) -> Option<MusicAbstract> {
        if let Some(PlayerMedia { queue, index, .. }) = self.current.read().unwrap().as_ref() {
            if self.can_play_next() {
                let next_index = if *index + 1 >= queue.len() {
                    0
                } else {
                    *index + 1
                };
                if let Some(music) = queue.get(next_index) {
                    return Some(music.clone());
                }
            }
        }
        Default::default()
    }
}

pub(crate) fn notify_player_current(cx: &Arc<BackendContext>) -> BResult<()> {
    let current = get_player_current(cx)?;
    cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Current {
        value: current,
    }));
    Ok(())
}

fn preload_next_music(cx: &Arc<BackendContext>) -> BResult<()> {
    if let Some(music) = cx.player_state().next_music() {
        cx.asset_server().schedule_preload(cx, music.id())?;
    }
    Ok(())
}

pub(crate) async fn player_request_play(
    cx: &Arc<BackendContext>,
    to_play: PlayerMedia,
) -> BResult<()> {
    let prev_music = {
        let current = cx.player_state().current.read().unwrap();
        current.clone()
    };

    if prev_music.is_some()
        && prev_music.as_ref().map(|v| v.id).unwrap() == to_play.id
        && prev_music.as_ref().map(|v| v.playlist_id).unwrap() == to_play.playlist_id
    {
        return Ok(());
    }

    let music = to_play.queue[to_play.index].clone();

    let item = MusicToPlay {
        id: to_play.id,
        url: cx.asset_server().serve_music_url(to_play.id),
        title: music.title().to_string(),
        has_cover: music.cover.is_some(),
    };
    {
        let cx = cx.clone();
        cx.async_runtime()
            .clone()
            .spawn_on_main(async move {
                cx.player_delegate().set_music_url(item);
                cx.player_delegate().resume();

                {
                    let mut state = cx.player_state().current.write().unwrap();
                    *state = Some(to_play.clone());
                }
                notify_player_current(&cx).unwrap();
                preload_next_music(&cx).unwrap();
            })
            .await;
    }
    Ok(())
}

pub(crate) async fn player_request_play_adjacent<const IS_NEXT: bool>(
    cx: &Arc<BackendContext>,
) -> BResult<()> {
    let (music, can_play) = {
        let state = cx.player_state();
        (
            state.current.read().unwrap().clone(),
            if IS_NEXT {
                state.can_play_next()
            } else {
                state.can_play_previous()
            },
        )
    };

    if music.is_none() {
        return Ok(());
    }
    let PlayerMedia {
        id: current_music_id,
        playlist_id,
        queue,
        ..
    } = music.unwrap();

    if !can_play {
        return Ok(());
    }

    let current_index = queue
        .iter()
        .position(|m| m.id() == current_music_id)
        .unwrap_or(0);
    let adjacent_index = if IS_NEXT {
        if current_index + 1 >= queue.len() {
            0
        } else {
            current_index + 1
        }
    } else {
        if current_index == 0 {
            queue.len() - 1
        } else {
            current_index - 1
        }
    };
    if let Some(adjacent_music) = queue.get(adjacent_index) {
        let to_play = PlayerMedia {
            id: adjacent_music.id(),
            playlist_id,
            queue,
            index: adjacent_index,
        };

        player_request_play(cx, to_play).await?;
    }
    Ok(())
}

pub(crate) fn player_clear_current(cx: &Arc<BackendContext>) {
    cx.player_state().current.write().unwrap().take();
}

pub(crate) async fn player_refresh_current(cx: &Arc<BackendContext>) -> BResult<()> {
    let current = cx.player_state().current.write().unwrap().clone();

    if current.is_none() {
        return Ok(());
    }

    let current = current.unwrap();

    let playlist = get_playlist(&cx, current.playlist_id).await?;
    if let Some(playlist) = playlist {
        let pos = playlist.musics.iter().position(|v| v.id() == current.id);
        if let Some(pos) = pos {
            let mut copied = current.clone();
            copied.index = pos;
            copied.queue = Arc::new(playlist.musics);

            {
                let mut w = cx.player_state().current.write().unwrap();
                *w = Some(copied);
            }
            notify_player_current(&cx)?;

            return Ok::<_, BError>(());
        }
    }

    {
        let cx = cx.clone();
        cx.async_runtime()
            .clone()
            .spawn_on_main(async move {
                cx.player_delegate().stop();
            })
            .await
    };
    player_clear_current(&cx);
    notify_player_current(&cx)?;
    return Ok(());
}

pub(crate) fn get_player_current(
    cx: &Arc<BackendContext>,
) -> BResult<Option<PlayerCurrentPlaying>> {
    let player_state = cx.player_state();
    let state = player_state.current.read().unwrap().clone();
    if state.is_none() {
        return Ok(None);
    }
    let playmode = *player_state.playmode.read().unwrap();
    let state = state.unwrap();

    let current = PlayerCurrentPlaying {
        abstr: state.queue[state.index].clone(),
        playlist_id: state.playlist_id,
        index: state.index,
        mode: playmode,
        can_prev: player_state.can_play_previous(),
        can_next: player_state.can_play_next(),
        prev_cover: player_state
            .prev_music()
            .map(|v| v.cover)
            .unwrap_or_default(),
        next_cover: player_state
            .next_music()
            .map(|v| v.cover)
            .unwrap_or_default(),
        cover: state.queue[state.index].cover.clone(),
    };
    Ok(Some(current))
}

pub(crate) async fn on_player_event(
    cx: &Arc<BackendContext>,
    event: PlayerDelegateEvent,
) -> BResult<()> {
    let cx = cx.clone();
    let rt = cx.async_runtime().clone();
    match event {
        PlayerDelegateEvent::Complete => {
            let play_mode = *cx.player_state().playmode.read().unwrap();
            match play_mode {
                PlayMode::Single => {
                    rt.spawn_on_main(async move {
                        cx.player_delegate().pause();
                        cx.player_delegate().seek(0);
                    })
                    .await;
                }
                PlayMode::SingleLoop => {
                    rt.spawn_on_main(async move {
                        cx.player_delegate().pause();
                        cx.player_delegate().seek(0);
                        cx.player_delegate().resume();
                    })
                    .await;
                }
                PlayMode::List | PlayMode::ListLoop => {
                    player_request_play_adjacent::<true>(&cx).await?;
                }
            }
        }
        PlayerDelegateEvent::Pause => {
            cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Playing {
                value: false,
            }));
        }
        PlayerDelegateEvent::Play => {
            cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Playing {
                value: true,
            }));
        }
        PlayerDelegateEvent::Seek => {
            cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Seeked));
        }
        PlayerDelegateEvent::Loading => {
            cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Loading));
        }
        PlayerDelegateEvent::Loaded => {
            cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Loaded));
        }
        PlayerDelegateEvent::Stop => {
            player_clear_current(&cx);
            notify_player_current(&cx)?;
        }
        PlayerDelegateEvent::Total { id, duration_ms } => {
            update_music_duration(
                &cx,
                ArgUpdateMusicDuration {
                    id,
                    duration: MusicDuration::new(Duration::from_millis(duration_ms)),
                },
            )
            .await?
        }
        PlayerDelegateEvent::Cover { id, buffer } => {
            update_music_cover(&cx, ArgUpdateMusicCover { id, cover: buffer }).await?
        }
        PlayerDelegateEvent::Error { msg } => {
            cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Error {
                value: msg,
            }));
        }
    }
    Ok(())
}

pub(crate) async fn on_connect_for_player(
    cx: &Arc<BackendContext>,
    playmode: PlayMode,
) -> BResult<()> {
    {
        let mut w = cx.player_state().playmode.write().unwrap();
        *w = playmode;
    }

    cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Current {
        value: get_player_current(cx)?,
    }));
    {
        let cx = cx.clone();
        cx.async_runtime()
            .clone()
            .spawn_on_main(async move {
                cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Playing {
                    value: cx.player_delegate().is_playing(),
                }));
            })
            .await;
    }
    cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Playmode {
        value: playmode,
    }));
    Ok(())
}
