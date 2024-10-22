use std::collections::{HashMap, HashSet};

use ease_client_shared::{
    backends::storage::{
        ArgUpsertStorage, Storage, StorageConnectionTestResult, StorageEntry, StorageId,
    },
    uis::storage::{CurrentStorageImportType, CurrentStorageStateType},
};

#[derive(Default, Clone)]
pub struct AllStorageState {
    pub storages: HashMap<StorageId, Storage>,
    pub storage_ids: Vec<StorageId>,
}

#[derive(Default, Clone)]
pub struct EditStorageState {
    pub is_create: bool,
    pub title: String,
    pub info: ArgUpsertStorage,
    pub test: StorageConnectionTestResult,
}

#[derive(Default, Clone)]
pub struct CurrentStorageState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub entries: Vec<StorageEntry>,
    pub checked_entries_path: HashSet<String>,
    pub current_storage_id: Option<StorageId>,
    pub current_path: String,
}
