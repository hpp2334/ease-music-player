use std::collections::{HashMap, HashSet};

use ease_client_shared::{
    backends::storage::{
        ArgUpsertStorage, Storage, StorageConnectionTestResult, StorageEntry, StorageId,
    },
    uis::storage::{CurrentStorageImportType, CurrentStorageStateType},
};
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum FormFieldStatus {
    #[default]
    Ok,
    CannotBeEmpty,
}

#[derive(Debug, Default, Clone, Serialize, uniffi::Record)]
pub struct EditStorageFormValidated {
    pub address: FormFieldStatus,
    pub username: FormFieldStatus,
    pub password: FormFieldStatus,
}

#[derive(Default, Clone)]
pub struct AllStorageState {
    pub storages: HashMap<StorageId, Storage>,
    pub storage_ids: Vec<StorageId>,
    pub local_storage_path: String,
}

#[derive(Default, Clone)]
pub struct EditStorageState {
    pub is_create: bool,
    pub info: ArgUpsertStorage,
    pub validated: EditStorageFormValidated,
    pub test: StorageConnectionTestResult,
    pub music_count: u32,
    pub playlist_count: u32,
}

#[derive(Default, Clone)]
pub struct CurrentStorageState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub entries: Vec<StorageEntry>,
    pub checked_entries_path: HashSet<String>,
    pub current_storage_id: Option<StorageId>,
    pub current_path: String,
    pub undo_stack: Vec<String>,
}

impl EditStorageFormValidated {
    pub fn is_valid(&self) -> bool {
        self.address == FormFieldStatus::Ok
            && self.username == FormFieldStatus::Ok
            && self.password == FormFieldStatus::Ok
    }
}

impl CurrentStorageState {
    pub fn checked_entries(&self) -> Vec<StorageEntry> {
        self.entries
            .iter()
            .filter(|entry| self.checked_entries_path.contains(&entry.path))
            .cloned()
            .collect()
    }
}
