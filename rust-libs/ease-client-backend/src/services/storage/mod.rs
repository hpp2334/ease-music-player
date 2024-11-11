use std::{sync::Arc, time::Duration};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::storage::StorageModel,
    repositories::{
        core::get_conn, music::db_get_playlists_count_by_storage,
        playlist::db_get_musics_count_by_storage, storage::db_load_storage,
    },
};
use ease_client_shared::backends::storage::{ArgUpsertStorage, Storage, StorageId, StorageType};
use ease_remote_storage::{BuildWebdavArg, LocalBackend, StorageBackend, Webdav};
use num_traits::FromPrimitive;

pub fn build_storage(model: StorageModel, music_count: u32, playlist_count: u32) -> Storage {
    Storage {
        id: model.id,
        addr: model.addr,
        alias: model.alias,
        username: model.username,
        password: model.password,
        is_anonymous: model.is_anonymous,
        typ: StorageType::from_i32(model.typ).unwrap(),
        music_count,
        playlist_count,
    }
}

pub fn get_storage_backend_by_arg(
    cx: &BackendContext,
    arg: ArgUpsertStorage,
) -> BResult<Arc<dyn StorageBackend + Send + Sync>> {
    let connect_timeout = Duration::from_secs(5);

    let ret: Arc<dyn StorageBackend + Send + Sync + 'static> = match arg.typ {
        StorageType::Local => Arc::new(LocalBackend::new()),
        StorageType::Webdav => {
            let arg = BuildWebdavArg {
                addr: arg.addr,
                username: arg.username,
                password: arg.password,
                is_anonymous: arg.is_anonymous,
                connect_timeout,
            };
            Arc::new(Webdav::new(arg))
        }
    };
    Ok(ret)
}

pub fn get_storage_backend(
    cx: &BackendContext,
    storage_id: StorageId,
) -> BResult<Option<Arc<dyn StorageBackend + Send + Sync>>> {
    let conn = get_conn(&cx)?;
    let model = db_load_storage(conn.get_ref(), storage_id)?;
    let (music_count, playlist_count) = if let Some(model) = model.as_ref() {
        let music_count = db_get_musics_count_by_storage(conn.get_ref(), model.id)?;
        let playlist_count = db_get_playlists_count_by_storage(conn.get_ref(), model.id)?;
        (music_count, playlist_count)
    } else {
        (0, 0)
    };
    drop(conn);

    if model.is_none() {
        return Ok(None);
    }
    let storage = model.unwrap();
    let storage = build_storage(storage, music_count, playlist_count);
    let backend = get_storage_backend_by_arg(
        &cx,
        ArgUpsertStorage {
            id: None,
            addr: storage.addr,
            alias: storage.alias,
            username: storage.username,
            password: storage.password,
            is_anonymous: storage.is_anonymous,
            typ: storage.typ,
        },
    )?;
    Ok(Some(backend))
}
