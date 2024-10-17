use std::sync::{atomic::AtomicU16, Arc};

use tokio::sync::mpsc;

use crate::error::{BError, BResult};

#[derive(Clone)]
pub struct BackendContext {
    pub storage_path: String,
    pub app_document_dir: String,
    pub schema_version: u32,
    pub server_port: Arc<AtomicU16>,
}