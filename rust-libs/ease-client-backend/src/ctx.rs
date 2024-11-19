use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        atomic::{AtomicU16, AtomicU32, AtomicUsize},
        Arc, RwLock,
    },
    time::Duration,
};

use ease_client_shared::backends::connector::{ConnectorAction, IConnectorNotifier};
use misty_async::AsyncRuntime;

use crate::services::player::{IPlayerDelegate, PlayerState};

#[derive(Clone)]
pub struct BackendContext {
    storage_path: Arc<RwLock<String>>,
    app_document_dir: Arc<RwLock<String>>,
    schema_version: Arc<AtomicU32>,
    server_port: Arc<AtomicU16>,
    rt: Arc<AsyncRuntime>,
    player: Arc<dyn IPlayerDelegate>,
    player_state: Arc<PlayerState>,
    connectors: Arc<(
        RwLock<HashMap<usize, Arc<dyn IConnectorNotifier>>>,
        AtomicUsize,
    )>,
}
impl Debug for BackendContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendContext")
            .field("storage_path", &self.storage_path)
            .field("app_document_dir", &self.app_document_dir)
            .field("schema_version", &self.schema_version)
            .field("server_port", &self.server_port)
            .finish()
    }
}

impl BackendContext {
    pub fn new(rt: Arc<AsyncRuntime>, player: Arc<dyn IPlayerDelegate>) -> Self {
        Self {
            storage_path: Arc::new(RwLock::new(String::new())),
            app_document_dir: Arc::new(RwLock::new(String::new())),
            schema_version: Arc::new(AtomicU32::new(0)),
            server_port: Arc::new(AtomicU16::new(0)),
            rt,
            player_state: Default::default(),
            player,
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

    pub fn player_state(&self) -> &Arc<PlayerState> {
        &self.player_state
    }
    pub fn player_delegate(&self) -> &Arc<dyn IPlayerDelegate> {
        &self.player
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

    pub fn set_server_port(&self, v: u16) {
        self.server_port
            .store(v, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_server_port(&self) -> u16 {
        self.server_port.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn notify(&self, payload: ConnectorAction) {
        let connectors = self.connectors.0.read().unwrap();
        for (_, connector) in connectors.iter() {
            connector.notify(payload.clone());
        }
    }
}
