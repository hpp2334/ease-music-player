use std::sync::Arc;
use std::time::Duration;

use crate::services::music::{update_music_cover, ArgUpdateMusicCover, ArgUpdateMusicDuration};
use crate::services::player::{
    get_player_current, notify_player_current, player_clear_current, player_request_play,
    player_request_play_adjacent,
};
use crate::services::preference::save_preference_playmode;
use crate::{
    ctx::BackendContext,
    error::BResult,
    services::{music::update_music_duration, player::PlayerMedia, playlist::get_playlist},
};
use ease_client_shared::backends::player::{
    ConnectorPlayerAction, PlayMode, PlayerCurrentPlaying, PlayerDelegateEvent,
};
use ease_client_shared::backends::{
    connector::ConnectorAction, music_duration::MusicDuration, player::ArgPlayMusic,
};

pub(crate) async fn cp_player_current(
    cx: &BackendContext,
    _arg: (),
) -> BResult<Option<PlayerCurrentPlaying>> {
    get_player_current(cx)
}

pub(crate) async fn cp_player_playmode(cx: &BackendContext, _arg: ()) -> BResult<PlayMode> {
    let playmode = cx.player_state().playmode.read().unwrap();
    Ok(*playmode)
}

pub(crate) async fn cp_player_current_duration(
    cx: &BackendContext,
    _arg: (),
) -> BResult<std::time::Duration> {
    Ok(Duration::from_secs(
        cx.player_delegate().get_current_duration_s(),
    ))
}

pub(crate) async fn cp_play_music(cx: &BackendContext, arg: ArgPlayMusic) -> BResult<()> {
    let playlist = get_playlist(cx, arg.playlist_id).await?;
    if playlist.is_none() {
        tracing::warn!("play music but playlist {:?} not found", arg.playlist_id);
        return Ok(());
    }
    let playlist = playlist.unwrap();
    let current_index = playlist
        .musics
        .iter()
        .position(|m| m.id() == arg.id)
        .unwrap_or(0);
    let to_play = PlayerMedia {
        id: arg.id,
        playlist_id: playlist.id(),
        queue: Arc::new(playlist.musics.clone()),
        index: current_index,
    };

    player_request_play(cx, to_play)?;

    Ok(())
}

pub(crate) async fn cp_pause_player(cx: &BackendContext, _arg: ()) -> BResult<()> {
    cx.player_delegate().pause();
    Ok(())
}

pub(crate) async fn cp_play_next(cx: &BackendContext, _arg: ()) -> BResult<()> {
    player_request_play_adjacent::<true>(cx)
}

pub(crate) async fn cp_play_previous(cx: &BackendContext, _arg: ()) -> BResult<()> {
    player_request_play_adjacent::<false>(cx)
}

pub(crate) async fn cp_stop_player(cx: &BackendContext, _arg: ()) -> BResult<()> {
    cx.player_delegate().stop();
    Ok(())
}

pub(crate) async fn cp_player_seek(cx: &BackendContext, arg: u64) -> BResult<()> {
    cx.player_delegate().seek(arg);
    Ok(())
}

pub(crate) async fn cp_update_playmode(cx: &BackendContext, arg: PlayMode) -> BResult<()> {
    {
        let mut playmode = cx.player_state().playmode.write().unwrap();
        *playmode = arg;
    }

    save_preference_playmode(cx, arg);
    cx.notify(ConnectorAction::Player(ConnectorPlayerAction::Playmode {
        value: arg,
    }));
    notify_player_current(cx)?;
    Ok(())
}

pub(crate) async fn cp_resume_player(cx: &BackendContext, _arg: ()) -> BResult<()> {
    cx.player_delegate().resume();
    Ok(())
}

pub(crate) async fn cp_on_player_event(
    cx: &BackendContext,
    event: PlayerDelegateEvent,
) -> BResult<()> {
    match event {
        PlayerDelegateEvent::Complete => {
            let play_mode = *cx.player_state().playmode.read().unwrap();
            match play_mode {
                PlayMode::Single => {
                    cx.player_delegate().pause();
                    cx.player_delegate().seek(0);
                }
                PlayMode::SingleLoop => {
                    cx.player_delegate().pause();
                    cx.player_delegate().seek(0);
                    cx.player_delegate().resume();
                }
                PlayMode::List | PlayMode::ListLoop => {
                    player_request_play_adjacent::<true>(cx)?;
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
        PlayerDelegateEvent::Loading | PlayerDelegateEvent::Loaded => {}
        PlayerDelegateEvent::Stop => {
            player_clear_current(cx);
            notify_player_current(cx)?;
        }
        PlayerDelegateEvent::Total { id, duration_ms } => update_music_duration(
            cx,
            ArgUpdateMusicDuration {
                id,
                duration: MusicDuration::new(Duration::from_millis(duration_ms)),
            },
        )?,
        PlayerDelegateEvent::Cover { id, buffer } => {
            update_music_cover(cx, ArgUpdateMusicCover { id, cover: buffer })?
        }
    }
    Ok(())
}
