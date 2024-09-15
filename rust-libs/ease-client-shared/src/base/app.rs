#[derive(Debug, uniffi::Record)]
pub struct ArgInitializeApp {
    pub app_document_dir: String,
    pub schema_version: u32,
    pub storage_path: String,
}
