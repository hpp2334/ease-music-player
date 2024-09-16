use misty_serve::define_message;
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id};

define_id!(StorageId);

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub id: StorageId,
    pub addr: String,
    pub alias: Option<String>,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}

define_message!(UpsertStorageMsg, Code::UpsertStorage, ArgUpsertStorage, ());
define_message!(ListStorageMsg, Code::ListStorage, (), Vec<Storage>);
define_message!(GetStorageMsg, Code::GetStorage, StorageId, Option<Storage>);
define_message!(
    GetToRemoveStorageRefsMsg,
    Code::GetToRemoveStorageRefs,
    StorageId,
    Option<Storage>
);

define_message!(RemoveStorageMsg, Code::RemoveStorage, StorageId, ());
define_message!(TestStorageMsg, Code::TestStorage, ArgUpsertStorage, ());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ListStorageEntryChildrenResp {
    Ok(StorageEntry),
    AuthenticationFailed,
    Timeout,
}

define_message!(
    ListStorageEntryChildrenMsg,
    Code::ListStorageEntryChildren,
    StorageEntryLoc,
    ListStorageEntryChildrenResp
);
