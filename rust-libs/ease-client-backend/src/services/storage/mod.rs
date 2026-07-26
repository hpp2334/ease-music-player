use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    objects::{ArgUpsertStorage, Storage},
    services::{get_music, get_music_cover_bytes},
};
use ease_client_schema::{DataSourceKey, StorageEntryLoc, StorageId, StorageModel, StorageType};
use ease_remote_storage::{
    BuildOneDriveArg, BuildWebdavArg, LocalBackend, OneDriveBackend, StorageBackend, StreamFile,
    Webdav,
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
    let backend = get_storage_backend(cx, loc.storage_id).await?;
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

pub async fn get_storage_backend(
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

    // The synthetic Local storage is not persisted in the DB — build a
    // LocalBackend directly so browse and playback always succeed.
    if storage_id.is_local() {
        let backend = build_storage_backend_by_arg(
            cx,
            ArgUpsertStorage {
                id: None,
                addr: String::new(),
                alias: "Local".to_string(),
                username: String::new(),
                password: String::new(),
                is_anonymous: false,
                typ: StorageType::Local,
            },
        )?;
        let mut state = cx.storage_state().cache.write().unwrap();
        state.insert(storage_id, backend.clone());
        return Ok(Some(backend));
    }

    let model = cx.database_server().load_storage(storage_id).await?;
    let music_count = cx.database_server().load_storage_music_count(storage_id).await?;

    if model.is_none() {
        return Ok(None);
    }
    let storage = model.unwrap();
    let storage = build_storage(storage, music_count);
    let backend = build_storage_backend_by_arg(
        cx,
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
    let models = cx.database_server().load_storages().await?;

    let mut storages: Vec<Storage> = Vec::with_capacity(models.len() + 1);
    for m in models.into_iter() {
        // Local is always the synthetic sentinel-id entry injected below;
        // skip any DB-persisted Local row (e.g. carried over from a legacy
        // redb migration) so it is not shown twice.
        if m.typ == StorageType::Local {
            continue;
        }
        let music_count = cx.database_server().load_storage_music_count(m.id).await?;
        storages.push(build_storage(m, music_count));
    }

    // Always inject the synthetic Local storage so the biz layer can hit it
    // regardless of DB / migration state. See `StorageId::local`.
    storages.push(build_local_storage(cx).await?);

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

/// Build the synthetic, always-present Local storage. Reads the live music
/// count from the DB but is not itself a DB row — `StorageId::local()` is a
/// negative sentinel that never appears in the `storage` table.
pub async fn build_local_storage(cx: &BackendContext) -> BResult<Storage> {
    let local_id = StorageId::local();
    let local_count = cx.database_server().load_storage_music_count(local_id).await?;
    Ok(Storage {
        id: local_id,
        addr: String::new(),
        alias: "Local".to_string(),
        username: String::new(),
        password: String::new(),
        is_anonymous: false,
        typ: StorageType::Local,
        music_count: local_count,
    })
}

/// Create or update a storage row, rejecting attempts to write the synthetic
/// Local storage. Local is not persisted — it is always synthesized on read
/// by `list_storage` / `build_local_storage`, so persisting a row with
/// `typ = Local` would be a no-op from the user's perspective (the row gets
/// skipped on read) and would shadow the synthetic entry's intent.
pub async fn upsert_storage(cx: &BackendContext, arg: ArgUpsertStorage) -> BResult<StorageId> {
    if arg.typ == StorageType::Local {
        return Err(crate::error::BError::CustomError {
            message: "cannot create or update the synthetic Local storage".to_string(),
        });
    }
    let id = cx.database_server().upsert_storage(arg).await?;
    evict_storage_backend_cache(cx, id);
    Ok(id)
}

/// Remove a storage row, rejecting attempts to delete the synthetic Local
/// storage. Deleting Local would cascade-delete every music row whose
/// `loc_storage_id` matches the Local sentinel and detach them from all
/// playlists — catastrophic, and never what a caller intends.
pub async fn remove_storage(cx: &BackendContext, id: StorageId) -> BResult<()> {
    if id.is_local() {
        return Err(crate::error::BError::CustomError {
            message: "cannot remove the synthetic Local storage".to_string(),
        });
    }
    cx.database_server().remove_storage(id).await?;
    evict_storage_backend_cache(cx, id);
    Ok(())
}

async fn get_asset_file_by_loc(
    cx: &BackendContext,
    entry: StorageEntryLoc,
    byte_offset: u64,
) -> BResult<Option<StreamFile>> {
    let storage_backend = get_storage_backend(cx, entry.storage_id).await?;
    let Some(storage_backend) = storage_backend else {
        return Ok(None);
    };

    let file = storage_backend.get(entry.path, byte_offset).await;
    if let Err(e) = &file {
        if e.is_not_found() {
            return Ok(None);
        }
    }
    let file = file?;
    Ok(Some(file))
}

pub(crate) async fn get_asset_file(
    cx: &BackendContext,
    key: DataSourceKey,
    byte_offset: u64,
) -> BResult<Option<StreamFile>> {
    match key {
        DataSourceKey::Music { id } => {
            let m = get_music(cx, id).await?;
            let Some(m) = m else {
                return Ok(None);
            };
            get_asset_file_by_loc(cx, m.loc, byte_offset).await
        }
        DataSourceKey::Cover { id } => {
            let buf = get_music_cover_bytes(cx, id).await?;
            if buf.is_empty() {
                return Ok(None);
            }
            let file = StreamFile::new_from_bytes(buf.as_slice(), "Default", byte_offset);
            Ok(Some(file))
        }
        DataSourceKey::AnyEntry { entry } => get_asset_file_by_loc(cx, entry, byte_offset).await,
    }
}
