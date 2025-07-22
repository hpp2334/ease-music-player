#[derive(Debug, Clone, uniffi::Record)]
pub struct ArgInitializeApp {
    pub app_document_dir: String,
    pub app_cache_dir: String,
    pub storage_path: String,
}
