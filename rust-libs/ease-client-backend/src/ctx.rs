use std::{
    fmt::Debug,
    sync::{
        atomic::AtomicU32,
        Arc, OnceLock, RwLock, Weak,
    },
    time::Duration,
};

use ease_tur_rpc::RpcClient;
use ease_client_tokio::tokio_runtime;

use crate::{repositories::core::DatabaseServer, services::StorageState};

struct BackendContextInternal {
    storage_path: RwLock<String>,
    app_document_dir: RwLock<String>,
    schema_version: AtomicU32,
    storage_state: Arc<StorageState>,
    database_server: Arc<DatabaseServer>,
    /// Handle for invoking JS service-plugin handlers over the headless tur
    /// instance's event bus. Set once by the `wireServiceRpc` JNI trampoline
    /// after `createHeadlessInstance` + `loadModule`. `None` until then.
    service_rpc: OnceLock<RpcClient>,
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
                service_rpc: OnceLock::new(),
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

    /// Publish the JS service-plugin RPC handle. Called once from the
    /// `wireServiceRpc` JNI trampoline after the headless tur instance is
    /// created + the plugin module is loaded.
    pub fn set_service_rpc(&self, rpc: RpcClient) {
        let _ = self.internal.service_rpc.set(rpc);
    }

    /// The JS service-plugin RPC handle, if the headless instance is up.
    /// `JsStorageBackend` clones this for each plugin storage row.
    pub fn service_rpc(&self) -> Option<RpcClient> {
        self.internal.service_rpc.get().cloned()
    }

    /// The shared ease-client-tokio runtime handle (used by `JsStorageBackend`
    /// to spawn its chunk-bridging task).
    pub fn tokio_handle(&self) -> tokio::runtime::Handle {
        tokio_runtime().handle().clone()
    }
}
