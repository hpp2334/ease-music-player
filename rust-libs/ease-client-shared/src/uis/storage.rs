use serde::Serialize;

use crate::backends::storage::{
    ArgUpsertStorage, StorageConnectionTestResult, StorageEntryType, StorageId, StorageType,
};

#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq, uniffi::Enum)]
pub enum CurrentStorageImportType {
    #[default]
    Musics,
    EditPlaylistCover,
    CreatePlaylistEntries,
    CreatePlaylistCover,
    CurrentMusicLyrics,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum CurrentStorageStateType {
    #[default]
    Loading,
    OK,
    NeedPermission,
    AuthenticationFailed,
    Timeout,
    UnknownError,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VStorageListItem {
    pub storage_id: StorageId,
    pub name: String,
    pub sub_title: String,
    pub typ: StorageType,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VStorageListState {
    pub items: Vec<VStorageListItem>,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VCurrentStorageEntry {
    pub path: String,
    pub name: String,
    pub is_folder: bool,
    pub can_check: bool,
    pub checked: bool,
    pub entry_typ: StorageEntryType,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VSplitPathItem {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VCurrentStorageEntriesStateStorageItem {
    pub id: StorageId,
    pub name: String,
    pub subtitle: String,
    pub selected: bool,
    pub is_local: bool,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VCurrentStorageEntriesState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub current_storage_id: Option<StorageId>,
    pub storage_items: Vec<VCurrentStorageEntriesStateStorageItem>,
    pub entries: Vec<VCurrentStorageEntry>,
    pub selected_count: i32,
    pub split_paths: Vec<VSplitPathItem>,
    pub current_path: String,
    pub disabled_toggle_all: bool,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VEditStorageState {
    pub is_created: bool,
    pub title: String,
    pub info: ArgUpsertStorage,
    pub test: StorageConnectionTestResult,
    pub music_count: u32,
    pub playlist_count: u32,
    pub update_signal: u16,
}
