use std::sync::Arc;

use ease_client_shared::backends::music::{ArgUpdateMusicLyric, Music, MusicId};
use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::BResult,
    repositories::{core::get_conn, music::db_update_music_lyric},
    services::{
        music::{disable_time_to_pause, enable_time_to_pause, get_music, notify_music},
        storage::from_opt_storage_entry,
    },
};

pub(crate) async fn cr_get_music(cx: &Arc<BackendContext>, id: MusicId) -> BResult<Option<Music>> {
    get_music(&cx, id).await
}

pub(crate) async fn cu_update_music_lyric(
    cx: &Arc<BackendContext>,
    arg: ArgUpdateMusicLyric,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    let cover_loc = from_opt_storage_entry(arg.lyric_loc);
    db_update_music_lyric(conn.get_ref(), arg.id, cover_loc)?;

    try_join! {
        notify_music(cx, arg.id)
    }?;

    Ok(())
}

pub(crate) async fn cu_enable_time_to_pause(
    cx: &Arc<BackendContext>,
    arg: std::time::Duration,
) -> BResult<()> {
    enable_time_to_pause(cx, arg);
    Ok(())
}

pub(crate) async fn cu_disable_time_to_pause(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    disable_time_to_pause(cx);
    Ok(())
}
