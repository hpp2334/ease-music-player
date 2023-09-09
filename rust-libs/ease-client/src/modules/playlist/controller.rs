use misty_vm::controllers::MistyControllerContext;

use crate::modules::error::EaseResult;
use crate::modules::error::EASE_RESULT_NIL;
use crate::modules::music::MusicId;

use super::service::*;
use super::typ::*;

// Edit Playlist

pub fn controller_prepare_edit_playlist(
    ctx: MistyControllerContext,
    id: PlaylistId,
) -> EaseResult<()> {
    prepare_edit_playlist_state(ctx.handle(), id)?;
    return EASE_RESULT_NIL;
}

pub fn controller_finish_edit_playlist(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    finish_edit_playlist(ctx.handle())?;
    return EASE_RESULT_NIL;
}

pub fn controller_prepare_edit_playlist_cover(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    prepare_edit_playlist_cover(ctx.handle())?;
    return EASE_RESULT_NIL;
}

pub fn controller_update_edit_playlist_name(
    ctx: MistyControllerContext,
    arg: String,
) -> EaseResult<()> {
    update_edit_playlist_name(ctx.handle(), arg);
    return EASE_RESULT_NIL;
}

pub fn controller_clear_edit_playlist_cover(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    clear_edit_playlist_cover(ctx.handle());
    return EASE_RESULT_NIL;
}

pub fn controller_prepare_import_entries_in_current_playlist(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    prepare_import_entries_in_current_playlist(ctx.handle())?;
    Ok(())
}

// Create Playlist

pub fn controller_finish_create_playlist(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    finish_create_playlist(ctx.handle())?;
    return EASE_RESULT_NIL;
}

pub fn controller_clear_create_playlist(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    clear_create_playlist_state(ctx.handle());
    return EASE_RESULT_NIL;
}

pub fn controller_reset_create_playlist_full(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    reset_create_playlist_full(ctx.handle());
    return EASE_RESULT_NIL;
}

pub fn controller_prepare_create_playlist_cover(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    prepare_import_cover_in_create_playlist(ctx.handle())?;
    return EASE_RESULT_NIL;
}

pub fn controller_prepare_create_playlist_entries(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    prepare_import_entries_in_create_playlist(ctx.handle())?;
    return EASE_RESULT_NIL;
}

pub fn controller_prepare_create_playlist(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    clear_create_playlist_state(ctx.handle());
    return EASE_RESULT_NIL;
}

pub fn controller_update_create_playlist_name(
    ctx: MistyControllerContext,
    arg: String,
) -> EaseResult<()> {
    update_create_playlist_name(ctx.handle(), arg);
    return EASE_RESULT_NIL;
}

pub fn controller_clear_create_playlist_cover(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    clear_create_playlist_cover(ctx.handle());
    return EASE_RESULT_NIL;
}

pub fn controller_update_create_playlist_mode(
    ctx: MistyControllerContext,
    arg: CreatePlaylistMode,
) -> EaseResult<()> {
    update_create_playlist_mode(ctx.handle(), arg);
    Ok(())
}

// Common

pub fn controller_change_current_playlist(
    ctx: MistyControllerContext,
    playlist_id: PlaylistId,
) -> EaseResult<()> {
    change_current_playlist(ctx.handle(), playlist_id);
    Ok(())
}

pub fn controller_remove_playlist(ctx: MistyControllerContext, arg: PlaylistId) -> EaseResult<()> {
    remove_playlist(ctx.handle(), arg)?;
    return EASE_RESULT_NIL;
}

pub fn controller_remove_music_from_current_playlist(
    ctx: MistyControllerContext,
    id: MusicId,
) -> EaseResult<()> {
    remove_music_from_current_playlist(ctx.handle(), id)?;
    return EASE_RESULT_NIL;
}

pub fn controller_play_all_musics(ctx: MistyControllerContext, _arg: ()) -> EaseResult<()> {
    play_all_musics(ctx.handle())?;
    return EASE_RESULT_NIL;
}

pub fn controller_clear_edit_playlist_state(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    clear_edit_playlist_state(ctx.handle());
    return EASE_RESULT_NIL;
}
