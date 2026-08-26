use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ease_client_schema::entities::storage as storage_entity;
use ease_client_schema::{
    DataSourceKey, PluginId, PluginStorageId, StorageEntryLoc, StorageHandle, StorageId,
    StorageType,
};
use ease_js_storage::JsStorageBackend;
use ease_remote_storage::{LocalBackend, StorageBackend, StreamFile};
use tracing::instrument;

/// JSON shape storage plugins store under
/// `plugin_kv_single(plugin_id, "storage:<plugin_storage_id>")`:
/// `{ alias, secretId, ...provider-specific fields }`. Only `alias` is read
/// here (for the storage list).
#[derive(serde::Deserialize)]
struct PluginStorageMeta {
    #[serde(default)]
    alias: String,
}

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
    objects::Storage,
    services::{get_music, get_music_cover_bytes},
};

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
                // A stream-level failure (e.g. a JS storage-plugin surfacing
                // an HTTP error mid-stream, like a missing lyrics file) is a
                // missing entry, not a host failure — return `None`.
                match data.bytes().await {
                    Ok(data) => Ok(Some(data.to_vec())),
                    Err(e) => {
                        tracing::debug!("load_storage_entry_data stream failed: {e:?}");
                        Ok(None)
                    }
                }
            }
            Err(_) => Ok(None),
        };
        tracing::trace!("end load");
        ret
    } else {
        Ok(None)
    }
}

pub(crate) fn evict_storage_backend_cache(cx: &BackendContext, storage_id: StorageId) {
    let mut w = cx.storage_state().cache.write().unwrap();
    w.remove(&storage_id);
}

/// Resolve a `StorageId` to a live backend, dispatching on the registry row's
/// kind. Local -> `LocalBackend`; Plugin -> `JsStorageBackend` (WebDAV,
/// OneDrive, ... are all JS plugin providers).
pub async fn get_storage_backend(
    cx: &BackendContext,
    storage_id: StorageId,
) -> BResult<Option<Arc<dyn StorageBackend + Send + Sync>>> {
    {
        let state = cx.storage_state().cache.read().unwrap();
        if let Some(cached) = state.get(&storage_id) {
            return Ok(Some(cached.clone()));
        }
    }

    let ds = cx.database_server();
    let Some(row) = ds.load_storage_row(storage_id).await? else {
        return Ok(None);
    };

    let backend: Arc<dyn StorageBackend + Send + Sync + 'static> = match StorageType::from_i32(
        row.r#type,
    ) {
        Some(StorageType::Local) => Arc::new(LocalBackend::new()),
        Some(StorageType::Plugin) => {
            // A plugin storage references a JS service plugin instance. The
            // provider is the prefix of `plugin_storage_id` (e.g. `onedrive`
            // in `onedrive:<uuid>`); the full id is the `instance` carried in
            // every RPC. The JS handlers live under `<provider>:<op>`.
            let plugin_storage_id = row.plugin_storage_id.clone().unwrap_or_default();
            let (provider, instance) = match plugin_storage_id.split_once(':') {
                Some((p, rest)) if !p.is_empty() && !rest.is_empty() => {
                    (p.to_string(), plugin_storage_id.clone())
                }
                _ => {
                    return Err(BError::CustomError {
                        message: format!(
                            "plugin storage has malformed plugin_storage_id: {plugin_storage_id:?}"
                        ),
                    });
                }
            };
            let plugin_id = row.plugin_id.clone().unwrap_or_default();
            let rpc = cx.service_rpc_for(&plugin_id).ok_or_else(|| BError::CustomError {
                message: format!(
                    "plugin storage requested but service RPC is not wired for {plugin_id} (headless instance not up)"
                ),
            })?;
            Arc::new(JsStorageBackend::new(
                rpc,
                provider,
                instance,
                cx.tokio_handle(),
            ))
        }
        None => return Ok(None),
    };

    let mut state = cx.storage_state().cache.write().unwrap();
    state.insert(storage_id, backend.clone());
    Ok(Some(backend))
}

/// Build the UI-facing [`Storage`] from a registry row, joining the
/// kind-specific detail.
async fn build_storage_from_row(
    cx: &BackendContext,
    row: storage_entity::Model,
) -> BResult<Storage> {
    let id = StorageId::wrap(row.id);
    let music_count = cx.database_server().load_storage_music_count(id).await?;

    match StorageType::from_i32(row.r#type) {
        Some(StorageType::Local) => Ok(Storage {
            id,
            handle: StorageHandle::Local,
            alias: "Local".to_string(),
            music_count,
        }),
        Some(StorageType::Plugin) => {
            // Display alias is stored by the plugin under
            // `plugin_kv_single(plugin_id, "storage:<plugin_storage_id>")` as
            // JSON `{ alias, secretId, ... }` (written when the instance is
            // created).
            let plugin_id = row.plugin_id.unwrap_or_default();
            let plugin_storage_id = row.plugin_storage_id.unwrap_or_default();
            let kv_key = format!("storage:{plugin_storage_id}");
            let alias = cx
                .database_server()
                .plugin_kv_single_get(&plugin_id, &kv_key)
                .await
                .ok()
                .flatten()
                .and_then(|v| serde_json::from_str::<PluginStorageMeta>(&v).ok())
                .map(|m| m.alias)
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| plugin_storage_id.clone());
            Ok(Storage {
                id,
                handle: StorageHandle::Plugin {
                    plugin_id: PluginId::new(plugin_id),
                    plugin_storage_id: PluginStorageId::new(plugin_storage_id),
                },
                alias,
                music_count,
            })
        }
        None => Err(BError::CustomError {
            message: format!("unknown storage type discriminant: {}", row.r#type),
        }),
    }
}

pub async fn list_storage(cx: &BackendContext) -> BResult<Vec<Storage>> {
    let rows = cx.database_server().load_all_storage_rows().await?;
    let mut storages: Vec<Storage> = Vec::with_capacity(rows.len());
    for row in rows {
        storages.push(build_storage_from_row(cx, row).await?);
    }
    // Local first, then by id.
    storages.sort_by(|lhs, rhs| {
        let l_local = matches!(lhs.handle, StorageHandle::Local);
        let r_local = matches!(rhs.handle, StorageHandle::Local);
        if l_local != r_local {
            l_local.cmp(&r_local)
        } else {
            lhs.id.cmp(&rhs.id)
        }
    });
    Ok(storages)
}

/// Remove a storage registry row (+ cascade). Evicts the backend cache.
pub async fn remove_storage(cx: &BackendContext, id: StorageId) -> BResult<()> {
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
