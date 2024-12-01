use ease_client_shared::backends::storage::StorageId;

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct StorageModel {
    pub id: StorageId,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: i32,

    pub playlist_count: u32,
    pub storage_count: u32,
}
