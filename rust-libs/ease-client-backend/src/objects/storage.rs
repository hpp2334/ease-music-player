use ease_client_schema::{MusicId, PlaylistId, StorageEntryLoc, StorageHandle, StorageId};
use serde::{Deserialize, Serialize};

/// One entry in a directory listing, storage-kind-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageEntry {
    pub storage_id: StorageId,
    pub name: String,
    pub path: String,
    pub size: Option<u64>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageEntryType {
    Folder,
    Music,
    Image,
    Lyric,
    Other,
}

/// A storage source surfaced to the UI. `handle` identifies the kind; the
/// plugin-owned display `alias` comes from the plugin's kv config. Connection
/// details (WebDAV addr / credentials, ...) live entirely in the plugin's
/// kv + secret stores and never cross this boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub id: StorageId,
    pub handle: StorageHandle,
    pub alias: String,
    pub music_count: u64,
}

#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CurrentStorageImportType {
    #[default]
    None,
    ImportMusics {
        id: PlaylistId,
    },
    EditPlaylistCover,
    CreatePlaylistEntries,
    CreatePlaylistCover,
    CurrentMusicLyrics {
        id: MusicId,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CurrentStorageStateType {
    #[default]
    Loading,
    OK,
    NeedPermission,
    AuthenticationFailed,
    Timeout,
    UnknownError,
}

impl StorageEntry {
    pub fn loc(&self) -> StorageEntryLoc {
        StorageEntryLoc {
            path: self.path.clone(),
            storage_id: self.storage_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ListStorageEntryChildrenResp {
    Ok { data: Vec<StorageEntry> },
    AuthenticationFailed,
    Timeout,
    Unknown,
}

impl ListStorageEntryChildrenResp {
    pub fn is_error(&self) -> bool {
        match self {
            ListStorageEntryChildrenResp::Ok { .. } => false,
            ListStorageEntryChildrenResp::AuthenticationFailed => false,
            ListStorageEntryChildrenResp::Timeout => false,
            ListStorageEntryChildrenResp::Unknown => false,
        }
    }
}
