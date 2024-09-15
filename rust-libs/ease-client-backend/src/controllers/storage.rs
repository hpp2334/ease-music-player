use serde::{Deserialize, Serialize};

use crate::models::storage::StorageId;

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageEntry {
    pub path: String,
    pub storage_id: StorageId,
}
