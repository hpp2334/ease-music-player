use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use ease_client_schema::entities::storage as storage_entity;
use ease_client_schema::{
    DataSourceKey, PluginId, PluginStorageId, SecretId, SecretScope, StorageEntryLoc, StorageHandle,
    StorageId, StorageType, WebdavStorageId,
};
use ease_remote_storage::{BuildWebdavArg, LocalBackend, StorageBackend, StreamFile, Webdav};
use ease_js_storage::JsStorageBackend;
use tracing::instrument;

/// JSON shape the OneDrive plugin stores under
/// `plugin_kv_single(plugin_id, "storage:<plugin_storage_id>")`:
/// `{ alias, secretId }`. Only `alias` is read here (for the storage list).
#[derive(serde::Deserialize)]
struct PluginStorageMeta {
    #[serde(default)]
    alias: String,
}

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
    objects::{Storage, ArgUpsertWebdavStorage},
    repositories::secret::SecretStore,
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

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Build a WebDAV backend directly from connection params (no DB). Used by
/// `ct_test_storage` to validate a connection before persisting.
pub fn build_webdav_backend(
    addr: String,
    username: String,
    password: String,
    is_anonymous: bool,
) -> Arc<dyn StorageBackend + Send + Sync + 'static> {
    Arc::new(Webdav::new(BuildWebdavArg {
        addr,
        username,
        password,
        is_anonymous,
        connect_timeout: CONNECT_TIMEOUT,
    }))
}

pub(crate) fn evict_storage_backend_cache(cx: &BackendContext, storage_id: StorageId) {
    let mut w = cx.storage_state().cache.write().unwrap();
    w.remove(&storage_id);
}

/// Resolve a `StorageId` to a live backend, dispatching on the registry row's
/// kind. Local -> `LocalBackend`; Webdav -> load detail + internal secret ->
/// `Webdav`; Plugin -> `JsStorageBackend` (not yet wired — returns an error
/// until the plugin service-runtime hosting lands).
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
        Some(StorageType::Webdav) => {
            let wid = row.webdav_storage_id.ok_or_else(|| BError::CustomError {
                message: "webdav storage row missing webdav_storage_id".into(),
            })?;
            let w = ds
                .load_webdav_storage(WebdavStorageId::wrap(wid))
                .await?
                .ok_or_else(|| BError::CustomError {
                    message: "webdav_storage row missing".into(),
                })?;
            let password = match w.secret_id {
                Some(sid) => ds
                    .secret_get(SecretScope::Internal, SecretId::wrap(sid))
                    .await?
                    .unwrap_or_default(),
                None => String::new(),
            };
            build_webdav_backend(w.addr, w.username, password, w.is_anonymous != 0)
        }
        Some(StorageType::Plugin) => {
            // A plugin storage references a JS service plugin instance. The
            // provider is the prefix of `plugin_storage_id` (e.g. `onedrive`
            // in `onedrive:<uuid>`); the full id is the `instance` carried in
            // every RPC. The JS handlers live under `<provider>:<op>`.
            let plugin_storage_id = row.plugin_storage_id.clone().unwrap_or_default();
            let (provider, instance) = match plugin_storage_id.split_once(':') {
                Some((p, rest)) if !p.is_empty() && !rest.is_empty() => (p.to_string(), plugin_storage_id.clone()),
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
/// kind-specific detail (WebDAV addr/alias/etc. for WebDAV).
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
            addr: None,
            username: None,
            is_anonymous: None,
        }),
        Some(StorageType::Webdav) => {
            let wid = row.webdav_storage_id.ok_or_else(|| BError::CustomError {
                message: "webdav storage row missing webdav_storage_id".into(),
            })?;
            let w = cx
                .database_server()
                .load_webdav_storage(WebdavStorageId::wrap(wid))
                .await?
                .ok_or_else(|| BError::CustomError {
                    message: "webdav_storage row missing".into(),
                })?;
            Ok(Storage {
                id,
                handle: StorageHandle::Webdav {
                    webdav_storage_id: WebdavStorageId::wrap(wid),
                },
                alias: w.alias,
                music_count,
                addr: Some(w.addr),
                username: Some(w.username),
                is_anonymous: Some(w.is_anonymous != 0),
            })
        }
        Some(StorageType::Plugin) => {
            // Display alias is stored by the plugin under
            // `plugin_kv_single(plugin_id, "storage:<plugin_storage_id>")` as
            // JSON `{ alias, secretId }` (written during OAuth exchange).
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
                addr: None,
                username: None,
                is_anonymous: None,
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

/// Create or update a WebDAV storage (delegates the secret + detail rows to
/// the repository). Evicts the backend cache for the resulting id.
pub async fn upsert_webdav_storage(
    cx: &BackendContext,
    arg: ArgUpsertWebdavStorage,
) -> BResult<StorageId> {
    let arg = normalize_arg_upsert_webdav(arg);
    let id = cx.database_server().upsert_webdav_storage(arg).await?;
    evict_storage_backend_cache(cx, id);
    Ok(id)
}

/// Remove a storage registry row (+ cascade). Evicts the backend cache.
pub async fn remove_storage(cx: &BackendContext, id: StorageId) -> BResult<()> {
    cx.database_server().remove_storage(id).await?;
    evict_storage_backend_cache(cx, id);
    Ok(())
}

fn normalize_arg_upsert_webdav(mut arg: ArgUpsertWebdavStorage) -> ArgUpsertWebdavStorage {
    if arg.is_anonymous {
        arg.username = String::new();
        arg.password = String::new();
    }
    arg
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
