use std::sync::Arc;

use crate::{
    error::BResult,
    objects::{ArgUpdateMusicLyric, Music, MusicId},
    services::get_music,
    Backend,
};

#[uniffi::export]
pub async fn ct_get_music(cx: Arc<Backend>, id: MusicId) -> BResult<Option<Music>> {
    let cx = cx.get_context();
    get_music(cx, id).await
}

#[uniffi::export]
pub async fn ct_update_music_lyric(cx: Arc<Backend>, arg: ArgUpdateMusicLyric) -> BResult<()> {
    let cx = cx.get_context();
    cx.database_server()
        .update_music_lyric(arg.id, arg.lyric_loc)?;

    Ok(())
}
