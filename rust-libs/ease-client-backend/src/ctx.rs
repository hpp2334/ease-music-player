use std::sync::{atomic::AtomicU16, Arc};

#[derive(Clone)]
pub struct BackendGlobal {
    pub storage_path: String,
    pub app_document_dir: String,
    pub schema_version: u32,
    pub server_port: Arc<AtomicU16>,
}
