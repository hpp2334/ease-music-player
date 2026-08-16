use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        atomic::AtomicU32,
        Arc, RwLock, Weak,
    },
    time::Duration,
};

use ease_tur_rpc::RpcClient;
use ease_client_tokio::tokio_runtime;
use serde_json::Value;

use crate::{error::BResult, repositories::core::DatabaseServer, services::StorageState};

struct BackendContextInternal {
    storage_path: RwLock<String>,
    app_document_dir: RwLock<String>,
    schema_version: AtomicU32,
    storage_state: Arc<StorageState>,
    database_server: Arc<DatabaseServer>,
    /// Handles for invoking JS backend-plugin handlers over each headless
    /// tur instance's event bus, keyed by plugin id. Entries are added by
    /// the `wireServiceRpc` JNI trampoline after
    /// `createHeadlessInstance` + `loadModule` per plugin.
    service_rpcs: RwLock<HashMap<String, RpcClient>>,
}

impl Drop for BackendContextInternal {
    fn drop(&mut self) {
        tracing::info!("drop BackendContextInternal")
    }
}

#[derive(Clone)]
pub struct BackendContext {
    internal: Arc<BackendContextInternal>,
}

#[derive(Clone)]
pub struct WeakBackendContext {
    internal: Weak<BackendContextInternal>,
}

impl Debug for BackendContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendContext")
            .field("storage_path", &self.internal.storage_path)
            .field("app_document_dir", &self.internal.app_document_dir)
            .field("schema_version", &self.internal.schema_version)
            .finish()
    }
}

impl WeakBackendContext {
    pub fn upgrade(&self) -> Option<BackendContext> {
        self.internal.upgrade().map(|internal| BackendContext { internal })
    }
}

impl Default for BackendContext {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendContext {
    pub fn new() -> Self {
        Self {
            internal: Arc::new(BackendContextInternal {
                storage_path: RwLock::new(String::new()),
                app_document_dir: RwLock::new(String::new()),
                schema_version: AtomicU32::new(0),
                storage_state: Default::default(),
                database_server: DatabaseServer::new(),
                service_rpcs: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn weak(&self) -> WeakBackendContext {
        WeakBackendContext {
            internal: Arc::downgrade(&self.internal),
        }
    }

    pub fn current_time(&self) -> Duration {
        std::time::UNIX_EPOCH.elapsed().unwrap()
    }

    pub fn set_storage_path(&self, p: &str) {
        let mut w = self.internal.storage_path.write().unwrap();
        *w = p.to_string();
    }

    pub fn get_storage_path(&self) -> String {
        self.internal.storage_path.read().unwrap().clone()
    }

    pub(crate) fn storage_state(&self) -> &Arc<StorageState> {
        &self.internal.storage_state
    }

    pub(crate) fn database_server(&self) -> &Arc<DatabaseServer> {
        &self.internal.database_server
    }

    /// Publish the JS backend-plugin RPC handle for `plugin_id`. Called once
    /// per plugin from the `wireServiceRpc` JNI trampoline after the
    /// plugin's headless tur instance is created + its backend module loaded.
    pub fn set_service_rpc(&self, plugin_id: &str, rpc: RpcClient) {
        self.internal.service_rpcs.write().unwrap().insert(plugin_id.to_string(), rpc);
    }

    /// The JS backend-plugin RPC handle for `plugin_id`, if that plugin's
    /// headless instance is up. `JsStorageBackend` clones this for each
    /// plugin storage row; event dispatch targets it per plugin.
    pub fn service_rpc_for(&self, plugin_id: &str) -> Option<RpcClient> {
        self.internal.service_rpcs.read().unwrap().get(plugin_id).cloned()
    }

    /// Fire a `plugin.event` at one plugin's backend: push `{type, payload}`
    /// onto that plugin's dedicated event bus channel (tur #190 layout —
    /// [`ease_tur_rpc::EVENT_CHANNEL_ID`]), delivered to the JS `onEvent`
    /// registration. Fire-and-forget: no reply is sent, and a plugin with
    /// no registration silently never hears it.
    pub fn dispatch_plugin_event(
        &self,
        plugin_id: &str,
        event_type: &str,
        payload: Value,
    ) -> BResult<()> {
        let rpc = self.service_rpc_for(plugin_id).ok_or_else(|| {
            crate::error::BError::CustomError {
                message: format!(
                    "no service RPC wired for plugin {plugin_id} (headless instance not up)"
                ),
            }
        })?;
        rpc.emit_event(event_type, payload);
        Ok(())
    }

    /// The shared ease-client-tokio runtime handle (used by `JsStorageBackend`
    /// to spawn its chunk-bridging task).
    pub fn tokio_handle(&self) -> tokio::runtime::Handle {
        tokio_runtime().handle().clone()
    }
}
