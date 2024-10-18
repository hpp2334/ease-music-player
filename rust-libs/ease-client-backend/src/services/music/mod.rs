use ease_client_shared::backends::{
    music::{MusicAbstract, MusicId, MusicMeta},
    storage::StorageEntryLoc,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::music::MusicModel,
    repositories::{core::get_conn, music::db_load_music}, to_opt_storage_entry,
};

use super::server::loc::get_serve_url_from_opt_loc;

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
    }
}

pub(crate) fn build_music_abstract(cx: &BackendContext, model: MusicModel) -> MusicAbstract {
    let cover_loc = to_opt_storage_entry(model.picture_path.clone(), model.picture_storage_id);
    let cover_url = get_serve_url_from_opt_loc(&cx, cover_loc.clone());
    
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
