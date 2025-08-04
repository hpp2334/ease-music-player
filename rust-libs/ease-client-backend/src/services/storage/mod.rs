use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::StorageModel,
    objects::{ArgUpsertStorage, Storage, StorageEntryLoc, StorageId, StorageType},
};
use ease_remote_storage::{
    BuildOneDriveArg, BuildWebdavArg, LocalBackend, OneDriveBackend, StorageBackend, Webdav,
};
use tracing::instrument;

#[derive(Default)]
pub(crate) struct StorageState {
    cache: RwLock<HashMap<StorageId, Arc<dyn StorageBackend + Send + Sync + 'static>>>,
}

#[instrument]
pub(crate) async fn load_storage_entry_data(
    cx: &BackendContext,
    loc: &StorageEntryLoc,
) -> BResult<Option<Vec<u8>>> {
    let loc = loc.clone();
    let backend = get_storage_backend(cx, loc.storage_id)?;
    if let Some(backend) = backend {
        tracing::trace!("start load");
        let ret = match backend.get(loc.path, 0).await {
            Ok(data) => {
                let data = data.bytes().await.unwrap();
                let data = data.to_vec();
                Ok(Some(data))
            }
            Err(_) => Ok(None),
        };
        tracing::trace!("end load");
        ret
    } else {
        Ok(None)
    }
}

pub fn build_storage(model: StorageModel, music_count: u64) -> Storage {
    Storage {
        id: model.id,
        addr: model.addr,
        alias: model.alias,
        username: model.username,
        password: model.password,
        is_anonymous: model.is_anonymous,
        typ: model.typ,
        music_count,
    }
}

pub fn build_storage_backend_by_arg(
    _cx: &BackendContext,
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
        StorageType::OneDrive => {
            let arg = BuildOneDriveArg { code: arg.password };
            Arc::new(OneDriveBackend::new(arg))
        }
    };
    Ok(ret)
}

pub(crate) fn evict_storage_backend_cache(cx: &BackendContext, storage_id: StorageId) {
    let mut w = cx.storage_state().cache.write().unwrap();
    w.remove(&storage_id);
}

pub fn get_storage_backend(
    cx: &BackendContext,
    storage_id: StorageId,
) -> BResult<Option<Arc<dyn StorageBackend + Send + Sync>>> {
    {
        let state = cx.storage_state().cache.read().unwrap();
        let cached = state.get(&storage_id);
        if let Some(cached) = cached {
            return Ok(Some(cached.clone()));
        }
    }

    let model = cx.database_server().load_storage(storage_id)?;
    let music_count = cx.database_server().load_storage_music_count(storage_id)?;

    if model.is_none() {
        return Ok(None);
    }
    let storage = model.unwrap();
    let storage = build_storage(storage, music_count);
    let backend = build_storage_backend_by_arg(
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

    {
        let mut state = cx.storage_state().cache.write().unwrap();
        state.insert(storage_id, backend.clone());
    }
    Ok(Some(backend))
}

pub async fn list_storage(cx: &BackendContext) -> BResult<Vec<Storage>> {
    let models = cx.database_server().load_storages()?;

    let mut storages: Vec<Storage> = Default::default();
    for m in models.into_iter() {
        let music_count = cx.database_server().load_storage_music_count(m.id)?;

        storages.push(build_storage(m, music_count));
    }

    storages.sort_by(|lhs, rhs| {
        let l_local = lhs.typ == StorageType::Local;
        let r_local = rhs.typ == StorageType::Local;

        if l_local != r_local {
            l_local.cmp(&r_local)
        } else {
            lhs.id.cmp(&rhs.id)
        }
    });

    Ok(storages)
}
