use ease_client_shared::{MusicDuration, MusicId};
use serde::{Deserialize, Serialize};

use crate::{
    ctx::Context,
    define_message,
    models::music::MusicModel,
    repositories::{
        core::get_conn,
        music::{
            db_load_music, db_update_music_cover, db_update_music_lyric,
            db_update_music_total_duration,
        },
    },
    services::lyrics::{parse_lrc, Lyrics},
};

use super::storage::{
    from_opt_storage_entry, load_storage_entry_data, to_opt_storage_entry, StorageEntryLoc,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicMeta {
    pub id: MusicId,
    pub title: String,
    pub duration: Option<MusicDuration>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicLyric {
    pub loc: StorageEntryLoc,
    pub data: Lyrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Music {
    pub meta: MusicMeta,
    pub loc: StorageEntryLoc,
    pub picture_loc: Option<StorageEntryLoc>,
    pub lyric: Option<MusicLyric>,
}

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
    }
}

async fn load_lyric(cx: &Context, loc: Option<StorageEntryLoc>) -> Option<MusicLyric> {
    if loc.is_none() {
        return None;
    }
    let loc = loc.unwrap();
    let data = load_storage_entry_data(&cx, &loc).await;
    if let Err(e) = &data {
        tracing::error!("fail to load entry {:?}: {}", loc, e);
        return None;
    }
    let data = data.unwrap();
    if data.is_none() {
        return None;
    }
    let data = data.unwrap();
    let data = String::from_utf8_lossy(&data).to_string();
    let lyric = parse_lrc(data);
    if lyric.is_err() {
        let e = lyric.unwrap_err();
        tracing::error!("fail to parse lyric: {}", e);
        return None;
    }
    let lyric = lyric.unwrap();

    Some(MusicLyric { loc, data: lyric })
}

define_message!(GetMusicMsg, Code::GetMusic, MusicId, ());
pub(crate) async fn cr_get_music(cx: Context, id: MusicId) -> anyhow::Result<Option<Music>> {
    let conn = get_conn(&cx)?;
    let model = db_load_music(conn.get_ref(), id)?;
    if model.is_none() {
        return Ok(None);
    }

    let model = model.unwrap();
    let meta = build_music_meta(model.clone());
    let lyric_loc = to_opt_storage_entry(model.lyric_path, model.lyric_storage_id);
    let lyric = load_lyric(&cx, lyric_loc).await;

    let music: Music = Music {
        meta,
        loc: StorageEntryLoc {
            storage_id: model.storage_id,
            path: model.path,
        },
        picture_loc: to_opt_storage_entry(model.picture_path, model.picture_storage_id),
        lyric,
    };
    Ok(Some(music))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicDuration {
    id: MusicId,
    duration: MusicDuration,
}
define_message!(
    UpdateMusicDurationMsg,
    Code::UpdateMusicDuration,
    ArgUpdateMusicDuration,
    ()
);
pub(crate) async fn cu_update_music_duration(
    cx: Context,
    arg: ArgUpdateMusicDuration,
) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    db_update_music_total_duration(conn.get_ref(), arg.id, arg.duration)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicCover {
    id: MusicId,
    cover_loc: Option<StorageEntryLoc>,
}
define_message!(
    UpdateMusicCoverMsg,
    Code::UpdateMusicCover,
    ArgUpdateMusicCover,
    ()
);
pub(crate) async fn cu_update_music_cover(
    cx: Context,
    arg: ArgUpdateMusicCover,
) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let cover_loc = from_opt_storage_entry(arg.cover_loc);
    db_update_music_cover(conn.get_ref(), arg.id, cover_loc)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicLyric {
    id: MusicId,
    lyric_loc: Option<StorageEntryLoc>,
}
define_message!(
    UpdateMusicLyricMsg,
    Code::UpdateMusicLyric,
    ArgUpdateMusicLyric,
    ()
);
pub(crate) async fn cu_update_music_lyric(
    cx: Context,
    arg: ArgUpdateMusicLyric,
) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let cover_loc = from_opt_storage_entry(arg.lyric_loc);
    db_update_music_lyric(conn.get_ref(), arg.id, cover_loc)?;
    Ok(())
}
