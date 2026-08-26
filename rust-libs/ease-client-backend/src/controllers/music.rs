use ease_client_tokio::tokio_runtime;
use std::sync::Arc;

use ease_client_schema::MusicId;

use crate::{
    error::BResult,
    objects::Music,
    services::{
        get_music, get_music_abstract, update_music_cover, update_music_duration,
        ArgUpdateMusicCover, ArgUpdateMusicDuration, ArgUpdateMusicLyric,
    },
    Backend, MusicAbstract,
};

pub async fn ct_get_music(cx: Arc<Backend>, id: MusicId) -> BResult<Option<Music>> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            get_music(cx, id).await
        })
        .await
        .unwrap()
}

pub fn cts_get_music_abstract(cx: Arc<Backend>, id: MusicId) -> BResult<Option<MusicAbstract>> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { get_music_abstract(&cx, id).await })
}

pub async fn ct_update_music_lyric(cx: Arc<Backend>, arg: ArgUpdateMusicLyric) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .update_music_lyric(arg.id, arg.lyric_loc)
                .await?;

            Ok(())
        })
        .await
        .unwrap()
}

pub fn cts_update_music_duration(cx: Arc<Backend>, arg: ArgUpdateMusicDuration) -> BResult<()> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { update_music_duration(&cx, arg).await })
}

pub fn cts_update_music_cover(cx: Arc<Backend>, arg: ArgUpdateMusicCover) -> BResult<()> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { update_music_cover(&cx, arg).await })
}
