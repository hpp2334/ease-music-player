use ease_client_shared::StorageId;
use serde::{Deserialize, Serialize};

pub type StorageEntryLocModel = (Option<String>, Option<StorageId>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageModel {
    pub id: StorageId,
    pub addr: String,
    pub alias: Option<String>,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: i32,
}
