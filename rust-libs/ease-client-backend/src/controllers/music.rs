use ease_client_shared::backends::music::{ArgUpdateMusicLyric, Music, MusicId};

use crate::{
    ctx::BackendContext,
    error::BResult,
    repositories::{core::get_conn, music::db_update_music_lyric},
    services::{music::get_music, storage::from_opt_storage_entry},
};

pub(crate) async fn cr_get_music(cx: &BackendContext, id: MusicId) -> BResult<Option<Music>> {
    let rt = cx.async_runtime();
    let cx = cx.clone();
    rt.spawn(async move { get_music(&cx, id).await }).await
}

pub(crate) async fn cu_update_music_lyric(
    cx: &BackendContext,
    arg: ArgUpdateMusicLyric,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    let cover_loc = from_opt_storage_entry(arg.lyric_loc);
    db_update_music_lyric(conn.get_ref(), arg.id, cover_loc)?;
    Ok(())
}
