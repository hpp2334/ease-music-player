use std::{
    sync::{
        atomic::{AtomicU16, AtomicU32},
        Arc, RwLock,
    },
    time::Duration,
};

use tokio::sync::mpsc;

use crate::error::{BError, BResult};

#[derive(Clone)]
pub struct BackendContext {
    storage_path: Arc<RwLock<String>>,
    app_document_dir: Arc<RwLock<String>>,
    schema_version: Arc<AtomicU32>,
    server_port: Arc<AtomicU16>,
}

impl BackendContext {
    pub fn new() -> Self {
        Self {
            storage_path: Arc::new(RwLock::new(String::new())),
            app_document_dir: Arc::new(RwLock::new(String::new())),
            schema_version: Arc::new(AtomicU32::new(0)),
            server_port: Arc::new(AtomicU16::new(0)),
        }
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
}
