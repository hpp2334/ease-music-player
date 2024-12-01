use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        atomic::{AtomicU32, AtomicUsize},
        Arc, RwLock, Weak,
    },
    time::Duration,
};

use ease_client_shared::backends::connector::{ConnectorAction, IConnectorNotifier};
use getset::Getters;
use misty_async::AsyncRuntime;

use crate::{
    repositories::core::DatabaseServer,
    services::{
        music::TimeToPauseState,
        player::{IPlayerDelegate, PlayerState},
        server::AssetServer,
        storage::StorageState,
    },
};

struct BackendContextInternal {
    storage_path: RwLock<String>,
    app_document_dir: RwLock<String>,
    schema_version: AtomicU32,
    rt: Arc<AsyncRuntime>,
    player_delegate: Arc<dyn IPlayerDelegate>,
    player_state: Arc<PlayerState>,
    storage_state: Arc<StorageState>,
    time_to_pause_state: Arc<TimeToPauseState>,
    asset_server: Arc<AssetServer>,
    database_server: Arc<DatabaseServer>,
    connectors: (
        RwLock<HashMap<usize, Arc<dyn IConnectorNotifier>>>,
        AtomicUsize,
    ),
}

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
        if let Some(internal) = self.internal.upgrade() {
            Some(BackendContext { internal })
        } else {
            None
        }
    }
}

impl BackendContext {
    pub fn new(rt: Arc<AsyncRuntime>, player: Arc<dyn IPlayerDelegate>) -> Self {
        Self {
            internal: Arc::new(BackendContextInternal {
                storage_path: RwLock::new(String::new()),
                app_document_dir: RwLock::new(String::new()),
                schema_version: AtomicU32::new(0),
                rt,
                player_state: Default::default(),
                player_delegate: player,
                storage_state: Default::default(),
                time_to_pause_state: Default::default(),
                asset_server: AssetServer::new(),
                database_server: DatabaseServer::new(),
                connectors: Default::default(),
            }),
        }
    }

    pub fn weak(&self) -> WeakBackendContext {
        WeakBackendContext {
            internal: Arc::downgrade(&self.internal),
        }
    }

    pub fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        let id = self
            .internal
            .connectors
            .1
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.internal
            .connectors
            .0
            .write()
            .unwrap()
            .insert(id, notifier);
        id
    }

    pub fn disconnect(&self, handle: usize) {
        self.internal.connectors.0.write().unwrap().remove(&handle);
    }

    pub fn async_runtime(&self) -> &Arc<AsyncRuntime> {
        &self.internal.rt
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

    pub fn set_app_document_dir(&self, p: &str) {
        let mut w = self.internal.app_document_dir.write().unwrap();
        *w = p.to_string();
    }

    pub fn get_app_document_dir(&self) -> String {
        self.internal.app_document_dir.read().unwrap().clone()
    }

    pub fn set_schema_version(&self, v: u32) {
        self.internal
            .schema_version
            .store(v, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_schema_version(&self) -> u32 {
        self.internal
            .schema_version
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn notify(&self, payload: ConnectorAction) {
        let connectors = self.internal.connectors.0.read().unwrap();
        for (_, connector) in connectors.iter() {
            connector.notify(payload.clone());
        }
    }

    pub(crate) fn player_delegate(&self) -> &Arc<dyn IPlayerDelegate> {
        &self.internal.player_delegate
    }

    pub(crate) fn player_state(&self) -> &Arc<PlayerState> {
        &self.internal.player_state
    }

    pub(crate) fn storage_state(&self) -> &Arc<StorageState> {
        &self.internal.storage_state
    }

    pub(crate) fn time_to_pause_state(&self) -> &Arc<TimeToPauseState> {
        &self.internal.time_to_pause_state
    }

    pub(crate) fn asset_server(&self) -> &Arc<AssetServer> {
        &self.internal.asset_server
    }

    pub(crate) fn database_server(&self) -> &Arc<DatabaseServer> {
        &self.internal.database_server
    }
}
