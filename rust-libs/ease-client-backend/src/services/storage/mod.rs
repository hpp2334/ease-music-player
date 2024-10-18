use std::{sync::Arc, time::Duration};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::storage::StorageModel,
    repositories::{core::get_conn, storage::db_load_storage},
};
use ease_client_shared::backends::storage::{ArgUpsertStorage, Storage, StorageId, StorageType};
use ease_remote_storage::{BuildWebdavArg, StorageBackend, Webdav};
use num_traits::FromPrimitive;

pub fn build_storage(model: StorageModel) -> Storage {
    Storage {
        id: model.id,
        addr: model.addr,
        alias: model.alias,
        username: model.username,
        password: model.password,
        is_anonymous: model.is_anonymous,
        typ: StorageType::from_i32(model.typ).unwrap(),
        music_count: 0,
        playlist_count: 0,
    }
}

pub fn get_storage_backend_by_arg(
    cx: &BackendContext,
    arg: ArgUpsertStorage,
) -> BResult<Arc<dyn StorageBackend + Send + Sync>> {
    let connect_timeout = Duration::from_secs(5);

    let ret: Arc<dyn StorageBackend + Send + Sync + 'static> = match arg.typ {
        StorageType::Local => {
            unimplemented!()
        }
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
    drop(conn);

    if model.is_none() {
        return Ok(None);
    }
    let storage = model.unwrap();
    let storage = build_storage(storage);
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
