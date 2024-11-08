use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id, define_message};

define_id!(StorageId);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageEntryLoc {
    pub path: String,
    pub storage_id: StorageId,
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
    uniffi::Enum,
)]
pub enum StorageType {
    Local,
    #[default]
    Webdav,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    pub storage_id: StorageId,
    pub name: String,
    pub path: String,
    pub size: Option<usize>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, uniffi::Record)]
pub struct ArgUpsertStorage {
    pub id: Option<StorageId>,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, uniffi::Enum)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub id: StorageId,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
    pub music_count: u32,
    pub playlist_count: u32,
}

impl StorageEntry {
    pub fn loc(&self) -> StorageEntryLoc {
        StorageEntryLoc {
            path: self.path.clone(),
            storage_id: self.storage_id,
        }
    }
}

define_message!(UpsertStorageMsg, Code::UpsertStorage, ArgUpsertStorage, ());
define_message!(ListStorageMsg, Code::ListStorage, (), Vec<Storage>);
define_message!(GetStorageMsg, Code::GetStorage, StorageId, Option<Storage>);

define_message!(RemoveStorageMsg, Code::RemoveStorage, StorageId, ());
define_message!(
    TestStorageMsg,
    Code::TestStorage,
    ArgUpsertStorage,
    StorageConnectionTestResult
);

#[derive(Debug, Clone, Serialize, Deserialize)]
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

define_message!(
    ListStorageEntryChildrenMsg,
    Code::ListStorageEntryChildren,
    StorageEntryLoc,
    ListStorageEntryChildrenResp
);
