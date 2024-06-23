use std::time::Duration;

use misty_vm::controllers::MistyControllerContext;

use crate::modules::error::EaseResult;

use super::service::*;
use super::typ::*;

pub fn controller_play_music(ctx: MistyControllerContext, arg: MusicId) -> EaseResult<()> {
    play_music(ctx.handle(), arg)?;
    Ok(())
}

pub fn controller_pause_music(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    pause_music(ctx.handle());
    Ok(())
}

pub fn controller_resume_music(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    resume_music(ctx.handle());
    Ok(())
}

pub fn controller_stop_music(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    stop_music(ctx.handle());
    Ok(())
}

#[derive(Debug, uniffi::Record)]
pub struct ArgSeekMusic {
    pub duration: u64,
}

pub fn controller_seek_music(ctx: MistyControllerContext, arg: ArgSeekMusic) -> EaseResult<()> {
    seek_music(ctx.handle(), arg.duration);
    Ok(())
}

pub fn controller_set_current_music_position_for_player_internal(
    ctx: MistyControllerContext,
    arg: u64,
) -> EaseResult<()> {
    update_current_music_position(ctx.handle(), Duration::from_millis(arg));
    Ok(())
}

pub fn controller_update_current_music_total_duration_for_player_internal(
    ctx: MistyControllerContext,
    arg: u64,
) -> EaseResult<()> {
    update_current_music_total_duration(ctx.handle(), Duration::from_millis(arg))?;
    Ok(())
}

pub fn controller_update_current_music_playing_for_player_internal(
    ctx: MistyControllerContext,
    arg: bool,
) -> EaseResult<()> {
    update_current_music_playing(ctx.handle(), arg);
    Ok(())
}

pub fn controller_handle_play_music_event_for_player_internal(
    ctx: MistyControllerContext,
    arg: PlayMusicEventType,
) -> EaseResult<()> {
    handle_play_music_event_for_player_internal(ctx.handle(), arg)?;
    Ok(())
}

pub fn controller_play_next_music(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    play_next_music(ctx.handle())?;
    Ok(())
}

pub fn controller_play_previous_music(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    play_previous_music(ctx.handle())?;
    Ok(())
}

pub fn controller_update_music_playmode_to_next(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    update_music_play_mode_to_next(ctx.handle())?;
    Ok(())
}

pub fn controller_update_time_to_pause(
    ctx: MistyControllerContext,
    duration: u64,
) -> EaseResult<()> {
    update_time_to_pause(ctx.handle(), duration);
    Ok(())
}

pub fn controller_remove_time_to_pause(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    remove_time_to_pause(ctx.handle());
    Ok(())
}

pub fn controller_prepare_import_lyric(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    prepare_import_lyrics_to_current_music(ctx.handle())?;
    Ok(())
}

pub fn controller_remove_current_music_lyric(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    remove_current_music_lyric(ctx.handle())?;
    Ok(())
}

pub fn controller_change_to_current_music_playlist(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    change_to_current_music_playlist(ctx.handle())?;
    Ok(())
}
