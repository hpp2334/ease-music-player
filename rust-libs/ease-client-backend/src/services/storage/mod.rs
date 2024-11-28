use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    models::storage::{StorageEntryLocModel, StorageModel},
    repositories::{
        core::get_conn,
        music::db_get_playlists_count_by_storage,
        playlist::db_get_musics_count_by_storage,
        storage::{db_load_storage, db_load_storages},
    },
};
use ease_client_shared::backends::{
    connector::ConnectorAction,
    storage::{ArgUpsertStorage, Storage, StorageEntryLoc, StorageId, StorageType},
};
use ease_remote_storage::{
    BuildOneDriveArg, BuildWebdavArg, LocalBackend, OneDriveBackend, StorageBackend, Webdav,
};
use num_traits::FromPrimitive;
use tracing::instrument;

#[derive(Default)]
pub(crate) struct StorageState {
    cache: RwLock<HashMap<StorageId, Arc<dyn StorageBackend + Send + Sync + 'static>>>,
}

pub(crate) fn to_opt_storage_entry(
    path: Option<String>,
    id: Option<StorageId>,
) -> Option<StorageEntryLoc> {
    if path.is_some() && id.is_some() {
        Some(StorageEntryLoc {
            path: path.unwrap(),
            storage_id: id.unwrap(),
        })
    } else {
        None
    }
}

pub(crate) fn from_opt_storage_entry(loc: Option<StorageEntryLoc>) -> StorageEntryLocModel {
    if let Some(loc) = loc {
        (Some(loc.path), Some(loc.storage_id))
    } else {
        (None, None)
    }
}

#[instrument]
pub(crate) async fn load_storage_entry_data(
    cx: &Arc<BackendContext>,
    loc: &StorageEntryLoc,
) -> BResult<Option<Vec<u8>>> {
    let loc = loc.clone();
    let backend = get_storage_backend(cx, loc.storage_id)?;
    if let Some(backend) = backend {
        cx.async_runtime()
            .spawn(async move {
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
            })
            .await
    } else {
        Ok(None)
    }
}

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

pub fn build_storage_backend_by_arg(
    _cx: &Arc<BackendContext>,
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

pub(crate) fn evict_storage_backend_cache(cx: &Arc<BackendContext>, storage_id: StorageId) {
    let mut w = cx.storage_state().cache.write().unwrap();
    w.remove(&storage_id);
}

pub fn get_storage_backend(
    cx: &Arc<BackendContext>,
    storage_id: StorageId,
) -> BResult<Option<Arc<dyn StorageBackend + Send + Sync>>> {
    {
        let state = cx.storage_state().cache.read().unwrap();
        let cached = state.get(&storage_id);
        if let Some(cached) = cached {
            return Ok(Some(cached.clone()));
        }
    }

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

pub async fn list_storage(cx: &Arc<BackendContext>) -> BResult<Vec<Storage>> {
    let conn = get_conn(&cx)?;
    let models = db_load_storages(conn.get_ref())?;

    let mut storages: Vec<Storage> = Default::default();
    for m in models.into_iter() {
        let music_count = db_get_musics_count_by_storage(conn.get_ref(), m.id)?;
        let playlist_count = db_get_playlists_count_by_storage(conn.get_ref(), m.id)?;

        storages.push(build_storage(m, music_count, playlist_count));
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

pub async fn notify_storages(cx: &Arc<BackendContext>) -> BResult<()> {
    let storages = list_storage(cx).await?;
    cx.notify(ConnectorAction::Storages(storages));
    Ok(())
}
