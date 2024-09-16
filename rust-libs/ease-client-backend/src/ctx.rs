#[derive(Clone)]
pub struct Context {
    pub storage_path: String,
    pub app_document_dir: String,
    pub schema_version: u32,
    pub server_port: u16,
}
