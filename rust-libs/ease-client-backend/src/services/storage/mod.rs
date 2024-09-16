use std::{sync::Arc, time::Duration};

use crate::{
    ctx::Context,
    error::BResult,
    models::storage::StorageModel,
    repositories::{core::get_conn, storage::db_load_storage},
};
use ease_client_shared::backends::storage::{Storage, StorageId, StorageType};
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

pub fn get_storage_backend(
    cx: &Context,
    storage_id: StorageId,
) -> BResult<Option<Arc<dyn StorageBackend + Send + Sync>>> {
    let conn = get_conn(&cx)?;
    let storage = db_load_storage(conn.get_ref(), storage_id)?;
    drop(conn);

    if storage.is_none() {
        return Ok(None);
    }
    let storage = storage.unwrap();
    let storage = build_storage(storage);

    let connect_timeout = Duration::from_secs(5);
    let ret: Arc<dyn StorageBackend + Send + Sync + 'static> = match storage.typ {
        StorageType::Local => {
            unimplemented!()
        }
        StorageType::Webdav => {
            let arg = BuildWebdavArg {
                addr: storage.addr,
                username: storage.username,
                password: storage.password,
                is_anonymous: storage.is_anonymous,
                connect_timeout,
            };
            Arc::new(Webdav::new(arg))
        }
    };
    Ok(Some(ret))
}
