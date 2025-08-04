use crate::objects::{StorageId, StorageType};

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct StorageModel {
    pub id: StorageId,
    pub addr: String,
    pub alias: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: StorageType,
}
