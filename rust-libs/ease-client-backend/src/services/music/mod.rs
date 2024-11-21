use ease_client_shared::backends::{
    connector::ConnectorAction,
    music::{LyricLoadState, Music, MusicAbstract, MusicId, MusicLyric, MusicMeta},
    music_duration::MusicDuration,
    storage::StorageEntryLoc,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::music::MusicModel,
    repositories::{
        core::get_conn,
        music::{db_load_music, db_update_music_cover, db_update_music_total_duration},
    },
};

use super::{
    lyrics::parse_lrc,
    player::player_refresh_current,
    playlist::notify_all_playlist_abstracts,
    server::loc::{get_serve_cover_url_from_music_id, get_serve_url_from_music_id},
    storage::{load_storage_entry_data, to_opt_storage_entry},
};

async fn load_lyric(
    cx: &BackendContext,
    loc: Option<StorageEntryLoc>,
    is_fallback: bool,
) -> Option<MusicLyric> {
    let loc = match loc {
        Some(loc) => loc,
        None => {
            return None;
        }
    };
    let data = load_storage_entry_data(&cx, &loc).await;
    if let Err(e) = &data {
        tracing::error!("fail to load entry {:?}: {}", loc, e);
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: if is_fallback {
                LyricLoadState::Missing
            } else {
                LyricLoadState::Failed
            },
        });
    }
    let data = data.unwrap();
    if data.is_none() {
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: if is_fallback {
                LyricLoadState::Missing
            } else {
                LyricLoadState::Failed
            },
        });
    }
    let data = data.unwrap();
    let data = String::from_utf8_lossy(&data).to_string();
    let lyric = parse_lrc(data);
    if lyric.is_err() {
        let e = lyric.unwrap_err();
        tracing::error!("fail to parse lyric: {}", e);
        return Some(MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: LyricLoadState::Failed,
        });
    }
    let lyric = lyric.unwrap();

    Some(MusicLyric {
        loc,
        data: lyric,
        loaded_state: LyricLoadState::Loaded,
    })
}

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
    }
}

pub(crate) fn build_music_abstract(cx: &BackendContext, model: MusicModel) -> MusicAbstract {
    let cover_url = if model
        .cover
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or_default()
    {
        get_serve_cover_url_from_music_id(&cx, model.id)
    } else {
        Default::default()
    };

    MusicAbstract {
        cover_url,
        meta: build_music_meta(model),
    }
}

pub fn get_music_storage_entry_loc(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<StorageEntryLoc>> {
    let conn = get_conn(cx)?;
    let m = db_load_music(conn.get_ref(), id)?;
    if m.is_none() {
        return Ok(None);
    }
    let m = m.unwrap();
    let m = StorageEntryLoc {
        path: m.path,
        storage_id: m.storage_id,
    };
    Ok(Some(m))
}

pub fn get_music_cover_bytes(cx: &BackendContext, id: MusicId) -> BResult<Vec<u8>> {
    let conn = get_conn(cx)?;
    let m = db_load_music(conn.get_ref(), id)?;
    if m.is_none() {
        return Ok(Default::default());
    }
    let m = m.unwrap();
    let cover = m.cover.unwrap_or_default();
    Ok(cover)
}

pub(crate) struct ArgUpdateMusicDuration {
    pub id: MusicId,
    pub duration: MusicDuration,
}
pub(crate) async fn update_music_duration(
    cx: &BackendContext,
    arg: ArgUpdateMusicDuration,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_update_music_total_duration(conn.get_ref(), arg.id, arg.duration)?;
    player_refresh_current(cx).await?;
    cx.notify(ConnectorAction::MusicTotalDurationChanged(arg.id));
    notify_all_playlist_abstracts(&cx).await?;
    Ok(())
}

pub(crate) struct ArgUpdateMusicCover {
    pub id: MusicId,
    pub cover: Vec<u8>,
}
pub(crate) async fn update_music_cover(
    cx: &BackendContext,
    arg: ArgUpdateMusicCover,
) -> BResult<()> {
    let conn = get_conn(&cx)?;
    db_update_music_cover(conn.get_ref(), arg.id, arg.cover.clone())?;
    player_refresh_current(cx).await?;
    cx.notify(ConnectorAction::MusicCoverChanged(arg.id));
    notify_all_playlist_abstracts(&cx).await?;
    Ok(())
}

pub(crate) async fn get_music(cx: &BackendContext, id: MusicId) -> BResult<Option<Music>> {
    let conn = get_conn(&cx)?;
    let model = db_load_music(conn.get_ref(), id)?;
    if model.is_none() {
        return Ok(None);
    }

    let model = model.unwrap();
    let meta = build_music_meta(model.clone());
    let url = get_serve_url_from_music_id(&cx, meta.id);
    let loc = StorageEntryLoc {
        storage_id: model.storage_id,
        path: model.path,
    };
    let mut lyric_loc = to_opt_storage_entry(model.lyric_path, model.lyric_storage_id);
    let using_fallback = lyric_loc.is_none() && model.lyric_default;
    if using_fallback {
        lyric_loc = Some(StorageEntryLoc {
            path: {
                let mut path = loc.path.clone();
                let new_extension = ".lrc";
                if let Some(pos) = path.rfind('.') {
                    path.truncate(pos);
                }
                path.push_str(new_extension);
                path
            },
            storage_id: loc.storage_id,
        });
    }

    let lyric: Option<MusicLyric> = load_lyric(&cx, lyric_loc, using_fallback).await;
    let cover_url = if model.cover.unwrap_or_default().is_empty() {
        Default::default()
    } else {
        get_serve_cover_url_from_music_id(&cx, model.id)
    };

    let music: Music = Music {
        meta,
        loc,
        url,
        cover_url,
        lyric,
    };
    Ok(Some(music))
}

pub(crate) async fn notify_music(cx: &BackendContext, id: MusicId) -> BResult<()> {
    let music = get_music(cx, id).await?;
    if let Some(music) = music {
        cx.notify(ConnectorAction::Music(music));
    }
    Ok(())
}
