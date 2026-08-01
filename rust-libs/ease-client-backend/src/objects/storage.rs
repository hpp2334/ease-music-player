use ease_client_schema::{MusicId, PlaylistId, StorageEntryLoc, StorageHandle, StorageId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageEntry {
    pub storage_id: StorageId,
    pub name: String,
    pub path: String,
    pub size: Option<u64>,
    pub is_dir: bool,
}

/// Create / update a WebDAV storage. WebDAV-only: OneDrive is no longer a core
/// storage kind (it is a JS plugin provider); Local is always-present and not
/// user-createable. `password` is write-only plaintext (blank on edit = keep
/// the existing secret); it never appears on the returned [`Storage`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpsertWebdavStorage {
    /// Registry `StorageId` to update; `None` to create.
    pub id: Option<StorageId>,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageConnectionTestResult {
    #[default]
    None,
    Testing,
    Success,
    Unauthorized,
    Timeout,
    OtherError,
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
/// WebDAV-specific fields (`addr` / `username` / `is_anonymous`) are `Some`
/// only for WebDAV and `None` for Local / Plugin. The password is never
/// carried here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub id: StorageId,
    pub handle: StorageHandle,
    pub alias: String,
    pub music_count: u64,
    pub addr: Option<String>,
    pub username: Option<String>,
    pub is_anonymous: Option<bool>,
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
