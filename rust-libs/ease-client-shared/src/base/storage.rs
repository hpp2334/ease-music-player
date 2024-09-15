use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

use crate::StorageId;

#[derive(
    FromPrimitive,
    ToPrimitive,
    Serialize,
    Deserialize,
    Clone,
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
    pub name: String,
    pub path: String,
    pub size: Option<usize>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, uniffi::Record)]
pub struct ArgUpsertStorage {
    pub id: Option<StorageId>,
    pub addr: String,
    pub alias: Option<String>,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum StorageConnectionTestResult {
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
