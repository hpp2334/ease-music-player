use std::{sync::{atomic::AtomicU16, Arc}, time::Duration};

use tokio::sync::mpsc;

use crate::error::{BError, BResult};

#[derive(Clone)]
pub struct BackendContext {
    pub storage_path: String,
    pub app_document_dir: String,
    pub schema_version: u32,
    pub server_port: Arc<AtomicU16>,
}

impl BackendContext {
    pub fn current_time(&self) -> Duration {
        std::time::UNIX_EPOCH.elapsed().unwrap()
    }
}