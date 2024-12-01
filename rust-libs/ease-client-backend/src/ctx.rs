use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        atomic::{AtomicU32, AtomicUsize},
        Arc, RwLock,
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

#[derive(Getters)]
pub struct BackendContext {
    storage_path: RwLock<String>,
    app_document_dir: RwLock<String>,
    schema_version: AtomicU32,
    rt: Arc<AsyncRuntime>,
    #[getset(get = "pub(crate)")]
    player_delegate: Arc<dyn IPlayerDelegate>,
    #[getset(get = "pub(crate)")]
    player_state: Arc<PlayerState>,
    #[getset(get = "pub(crate)")]
    storage_state: Arc<StorageState>,
    #[getset(get = "pub(crate)")]
    time_to_pause_state: Arc<TimeToPauseState>,
    #[getset(get = "pub(crate)")]
    asset_server: Arc<AssetServer>,
    #[getset(get = "pub(crate)")]
    database_server: Arc<DatabaseServer>,
    connectors: (
        RwLock<HashMap<usize, Arc<dyn IConnectorNotifier>>>,
        AtomicUsize,
    ),
}
impl Debug for BackendContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendContext")
            .field("storage_path", &self.storage_path)
            .field("app_document_dir", &self.app_document_dir)
            .field("schema_version", &self.schema_version)
            .finish()
    }
}

impl BackendContext {
    pub fn new(rt: Arc<AsyncRuntime>, player: Arc<dyn IPlayerDelegate>) -> Self {
        Self {
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
        }
    }

    pub fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        let id = self
            .connectors
            .1
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.connectors.0.write().unwrap().insert(id, notifier);
        id
    }

    pub fn disconnect(&self, handle: usize) {
        self.connectors.0.write().unwrap().remove(&handle);
    }

    pub fn async_runtime(&self) -> &Arc<AsyncRuntime> {
        &self.rt
    }

    pub fn current_time(&self) -> Duration {
        std::time::UNIX_EPOCH.elapsed().unwrap()
    }

    pub fn set_storage_path(&self, p: &str) {
        let mut w = self.storage_path.write().unwrap();
        *w = p.to_string();
    }

    pub fn get_storage_path(&self) -> String {
        self.storage_path.read().unwrap().clone()
    }

    pub fn set_app_document_dir(&self, p: &str) {
        let mut w = self.app_document_dir.write().unwrap();
        *w = p.to_string();
    }

    pub fn get_app_document_dir(&self) -> String {
        self.app_document_dir.read().unwrap().clone()
    }

    pub fn set_schema_version(&self, v: u32) {
        self.schema_version
            .store(v, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_schema_version(&self) -> u32 {
        self.schema_version
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn notify(&self, payload: ConnectorAction) {
        let connectors = self.connectors.0.read().unwrap();
        for (_, connector) in connectors.iter() {
            connector.notify(payload.clone());
        }
    }
}
