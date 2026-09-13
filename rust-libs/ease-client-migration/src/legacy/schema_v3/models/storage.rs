use serde::{Deserialize, Serialize};

use super::super::objects::{StorageId, StorageType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageModel {
    pub id: StorageId,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}
