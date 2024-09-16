use ease_client_shared::backends::{
    music::{MusicId, MusicMeta},
    storage::StorageEntryLoc,
};

use crate::{
    ctx::Context,
    models::music::MusicModel,
    repositories::{core::get_conn, music::db_load_music},
    to_opt_storage_entry,
};

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
    }
}

pub fn get_music_storage_entry_loc(
    cx: &Context,
    id: MusicId,
) -> anyhow::Result<Option<StorageEntryLoc>> {
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
