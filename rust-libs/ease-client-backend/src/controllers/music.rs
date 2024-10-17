use ease_client_shared::backends::{
    music::{
        ArgUpdateMusicCover, ArgUpdateMusicDuration, ArgUpdateMusicLyric, Music, MusicId,
        MusicLyric,
    },
    storage::StorageEntryLoc,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    repositories::{
        core::get_conn,
        music::{
            db_load_music, db_update_music_cover, db_update_music_lyric,
            db_update_music_total_duration,
        },
    },
    services::{
        lyrics::parse_lrc,
        music::build_music_meta,
        server::loc::{get_serve_url_from_music_id, get_serve_url_from_opt_loc},
    },
};

use super::storage::{from_opt_storage_entry, load_storage_entry_data, to_opt_storage_entry};

async fn load_lyric(cx: &BackendContext, loc: Option<StorageEntryLoc>) -> Option<MusicLyric> {
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

pub(crate) async fn cr_get_music(cx: BackendContext, id: MusicId) -> BResult<Option<Music>> {
    let conn = get_conn(&cx)?;
    let model = db_load_music(conn.get_ref(), id)?;
    if model.is_none() {
        return Ok(None);
    }

    let model = model.unwrap();
    let meta = build_music_meta(model.clone());
    let url = get_serve_url_from_music_id(&cx, meta.id);
    let lyric_loc = to_opt_storage_entry(model.lyric_path, model.lyric_storage_id);
    let lyric: Option<MusicLyric> = load_lyric(&cx, lyric_loc).await;
    let cover_loc = to_opt_storage_entry(model.picture_path, model.picture_storage_id);
    let cover_url = get_serve_url_from_opt_loc(&cx, cover_loc.clone());

    let music: Music = Music {
        meta,
        loc: StorageEntryLoc {
            storage_id: model.storage_id,
            path: model.path,
        },
        url,
        cover_loc,
        cover_url,
        lyric,
    };
    Ok(Some(music))
}

pub(crate) async fn cu_update_music_duration(
    cx: BackendContext,
    arg: ArgUpdateMusicDuration,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_update_music_total_duration(conn.get_ref(), arg.id, arg.duration)?;
    Ok(())
}

pub(crate) async fn cu_update_music_cover(
    cx: BackendContext,
    arg: ArgUpdateMusicCover,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    let cover_loc = from_opt_storage_entry(arg.cover_loc);
    db_update_music_cover(conn.get_ref(), arg.id, cover_loc)?;
    Ok(())
}

pub(crate) async fn cu_update_music_lyric(
    cx: BackendContext,
    arg: ArgUpdateMusicLyric,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    let cover_loc = from_opt_storage_entry(arg.lyric_loc);
    db_update_music_lyric(conn.get_ref(), arg.id, cover_loc)?;
    Ok(())
}
