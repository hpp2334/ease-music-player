use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use ease_client_shared::backends::{
    music::MusicId,
    storage::{StorageEntryLoc, StorageId},
};
use ease_remote_storage::StreamFile;

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
    services::{
        music::{get_music_cover_bytes, get_music_storage_entry_loc},
        storage::get_storage_backend,
    },
};

pub(crate) async fn get_stream_file_by_loc(
    cx: &Arc<BackendContext>,
    loc: StorageEntryLoc,
) -> BResult<Option<StreamFile>> {
    let backend = get_storage_backend(&cx, loc.storage_id)?;
    if backend.is_none() {
        return Ok(None);
    }
    let backend = backend.unwrap();
    let stream_file = backend.get(&loc.path).await?;
    Ok(Some(stream_file))
}

pub(crate) async fn get_stream_file_by_music_id(
    cx: &Arc<BackendContext>,
    id: MusicId,
) -> BResult<Option<StreamFile>> {
    let loc = get_music_storage_entry_loc(&cx, id)?;
    if loc.is_none() {
        return Ok(None);
    }
    let loc = loc.unwrap();
    get_stream_file_by_loc(cx, loc).await
}

pub(crate) async fn get_stream_file_cover_by_music_id(
    cx: &Arc<BackendContext>,
    id: MusicId,
) -> BResult<Option<StreamFile>> {
    let bytes = get_music_cover_bytes(&cx, id)?;
    if !bytes.is_empty() {
        Ok(Some(StreamFile::new_from_bytes(bytes.as_slice(), "cover")))
    } else {
        Ok(None)
    }
}
