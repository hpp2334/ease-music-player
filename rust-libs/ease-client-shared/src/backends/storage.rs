use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

use crate::{backends::env::EASEM_ONEDRIVE_ID, define_id};

use super::{music::MusicId, playlist::PlaylistId};

define_id!(StorageId);
define_id!(BlobId);

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Hash,
    PartialEq,
    Eq,
    bitcode::Encode,
    bitcode::Decode,
    uniffi::Record,
    PartialOrd,
    Ord,
)]
pub struct StorageEntryLoc {
    pub storage_id: StorageId,
    pub path: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
    bitcode::Encode,
    bitcode::Decode,
    uniffi::Enum,
)]
pub enum DataSourceKey {
    Music { id: MusicId },
    Cover { id: MusicId },
    AnyEntry { entry: StorageEntryLoc },
}

#[derive(
    FromPrimitive,
    ToPrimitive,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Default,
    Hash,
    bitcode::Encode,
    bitcode::Decode,
    uniffi::Enum,
)]
pub enum StorageType {
    Local,
    #[default]
    Webdav,
    OneDrive,
}

#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct StorageEntry {
    pub storage_id: StorageId,
    pub name: String,
    pub path: String,
    pub size: Option<usize>,
    pub is_dir: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    bitcode::Encode,
    bitcode::Decode,
    uniffi::Record,
)]
pub struct ArgUpsertStorage {
    pub id: Option<StorageId>,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    bitcode::Encode,
    bitcode::Decode,
    uniffi::Enum,
)]
pub enum StorageConnectionTestResult {
    #[default]
    None,
    Testing,
    Success,
    Unauthorized,
    Timeout,
    OtherError,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, uniffi::Enum)]
pub enum StorageEntryType {
    Folder,
    Music,
    Image,
    Lyric,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct Storage {
    pub id: StorageId,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
    pub music_count: u64,
}

#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq, uniffi::Enum)]
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

impl StorageEntry {
    pub fn loc(&self) -> StorageEntryLoc {
        StorageEntryLoc {
            path: self.path.clone(),
            storage_id: self.storage_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub enum ListStorageEntryChildrenResp {
    Ok(Vec<StorageEntry>),
    AuthenticationFailed,
    Timeout,
    Unknown,
}

impl ListStorageEntryChildrenResp {
    pub fn is_error(&self) -> bool {
        match self {
            ListStorageEntryChildrenResp::Ok(_) => false,
            ListStorageEntryChildrenResp::AuthenticationFailed => false,
            ListStorageEntryChildrenResp::Timeout => false,
            ListStorageEntryChildrenResp::Unknown => false,
        }
    }
}

pub fn onedrive_oauth_url() -> String {
    let base_url = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
    let client_id: &str = EASEM_ONEDRIVE_ID;
    let redirect_uri = "easem://oauth2redirect/";
    let scope = urlencoding::encode("Files.Read offline_access").to_string();

    format!("{base_url}?client_id={client_id}&response_type=code&redirect_uri={redirect_uri}&scope={scope}")
}
