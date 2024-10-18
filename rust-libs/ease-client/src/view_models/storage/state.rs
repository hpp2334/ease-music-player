use std::collections::HashSet;

use ease_client_shared::{
    backends::storage::{StorageEntry, StorageId},
    uis::storage::{CurrentStorageImportType, CurrentStorageStateType},
};

#[derive(Default, Clone)]
pub struct CurrentStorageState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub entries: Vec<StorageEntry>,
    pub checked_entries_path: HashSet<String>,
    pub current_storage_id: Option<StorageId>,
    pub current_path: String,
}
