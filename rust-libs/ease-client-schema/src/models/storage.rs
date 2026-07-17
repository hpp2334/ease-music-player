use serde::{Deserialize, Serialize};

use crate::shared::{StorageId, StorageType};

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
