use std::sync::Arc;
use std::time::Duration;

use crate::services::music::{update_music_cover, ArgUpdateMusicCover, ArgUpdateMusicDuration};
use crate::services::player::{
    get_player_current, notify_player_current, on_player_event, player_clear_current,
    player_request_play, player_request_play_adjacent,
};
use crate::services::preference::save_preference_playmode;
use crate::{
    ctx::BackendContext,
    error::BResult,
    services::{music::update_music_duration, player::PlayerMedia, playlist::get_playlist},
};
use ease_client_shared::backends::player::{
    ConnectorPlayerAction, PlayMode, PlayerCurrentPlaying, PlayerDelegateEvent, PlayerDurations,
};
use ease_client_shared::backends::{
    connector::ConnectorAction, music_duration::MusicDuration, player::ArgPlayMusic,
};

pub(crate) async fn cp_player_current(
    cx: &Arc<BackendContext>,
    _arg: (),
) -> BResult<Option<PlayerCurrentPlaying>> {
    get_player_current(cx)
}

pub(crate) async fn cp_player_playmode(cx: &Arc<BackendContext>, _arg: ()) -> BResult<PlayMode> {
    let playmode = cx.player_state().playmode.read().unwrap();
    Ok(*playmode)
}

pub(crate) async fn cp_player_durations(
    cx: &Arc<BackendContext>,
    _arg: (),
) -> BResult<PlayerDurations> {
    let cx = cx.clone();
    cx.async_runtime()
        .clone()
        .spawn_on_main(async move { Ok(cx.player_delegate().get_durations()) })
        .await
}

pub(crate) async fn cp_play_music(cx: &Arc<BackendContext>, arg: ArgPlayMusic) -> BResult<()> {
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

    player_request_play(cx, to_play).await?;

    Ok(())
}

pub(crate) async fn cp_pause_player(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    let cx = cx.clone();
    cx.async_runtime()
        .clone()
        .spawn_on_main(async move {
            cx.player_delegate().pause();
        })
        .await;
    Ok(())
}

pub(crate) async fn cp_play_next(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    player_request_play_adjacent::<true>(cx).await
}

pub(crate) async fn cp_play_previous(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    player_request_play_adjacent::<false>(cx).await
}

pub(crate) async fn cp_stop_player(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    let cx = cx.clone();
    cx.async_runtime()
        .clone()
        .spawn_on_main(async move {
            cx.player_delegate().stop();
        })
        .await;
    Ok(())
}

pub(crate) async fn cp_player_seek(cx: &Arc<BackendContext>, arg: u64) -> BResult<()> {
    let cx = cx.clone();
    cx.async_runtime()
        .clone()
        .spawn_on_main(async move {
            cx.player_delegate().seek(arg);
        })
        .await;
    Ok(())
}

pub(crate) async fn cp_update_playmode(cx: &Arc<BackendContext>, arg: PlayMode) -> BResult<()> {
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

pub(crate) async fn cp_resume_player(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    let cx = cx.clone();
    cx.async_runtime()
        .clone()
        .spawn_on_main(async move {
            cx.player_delegate().resume();
        })
        .await;
    Ok(())
}

pub(crate) async fn cp_on_player_event(
    cx: &Arc<BackendContext>,
    event: PlayerDelegateEvent,
) -> BResult<()> {
    on_player_event(cx, event).await
}
