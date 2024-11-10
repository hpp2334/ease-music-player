use ease_client_shared::backends::{
    music::{MusicAbstract, MusicId, MusicMeta},
    storage::StorageEntryLoc,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::music::MusicModel,
    repositories::{core::get_conn, music::db_load_music},
    storage::to_opt_storage_entry,
};

use super::server::loc::{get_serve_cover_url_from_music_id, get_serve_url_from_opt_loc};

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
