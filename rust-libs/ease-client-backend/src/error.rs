use ease_client_schema::{MusicId, PlaylistId};
use serde::{Deserialize, Serialize};

/// Error type that flows across the JSON bridge.
///
/// Serializes via `#[serde(tag = "errorCode", content = "errorDetail")]`:
///
/// ```jsonc
/// // unit variant
/// { "errorCode": "AssetNotFound" }
/// // newtype variant
/// { "errorCode": "MusicNotFound", "errorDetail": { "value": 42 } }
/// // struct variant
/// { "errorCode": "CustomError", "errorDetail": { "message": "..." } }
/// ```
#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "errorCode", content = "errorDetail")]
pub enum BError {
    #[error("remote storage error: {0}")]
    RemoteStorageError(String),
    #[error("failed to load asset: {0}")]
    AssetLoadFail(String),
    #[error("asset not found")]
    AssetNotFound,
    #[error("playlist not found: {0:?}")]
    PlaylistNotFound(PlaylistId),
    #[error("music not found: {0:?}")]
    MusicNotFound(MusicId),
    #[error("database error: {0}")]
    DbError(String),
    #[error("io error: {0}")]
    IoError(String),
    #[error("json error: {0}")]
    JsonError(String),
    #[error("order key error: {0}")]
    OrderKeyError(String),
    #[error("custom: {message}")]
    CustomError { message: String },
    #[error("{0}")]
    AnyHowError(String),
}

impl From<ease_remote_storage::StorageBackendError> for BError {
    fn from(e: ease_remote_storage::StorageBackendError) -> Self {
        BError::RemoteStorageError(e.to_string())
    }
}

impl From<sea_orm::DbErr> for BError {
    fn from(e: sea_orm::DbErr) -> Self {
        BError::DbError(e.to_string())
    }
}

impl From<std::io::Error> for BError {
    fn from(e: std::io::Error) -> Self {
        BError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for BError {
    fn from(e: serde_json::Error) -> Self {
        BError::JsonError(e.to_string())
    }
}

impl From<ease_order_key::OrderKeyError> for BError {
    fn from(e: ease_order_key::OrderKeyError) -> Self {
        BError::OrderKeyError(e.to_string())
    }
}

impl From<anyhow::Error> for BError {
    fn from(e: anyhow::Error) -> Self {
        BError::AnyHowError(e.to_string())
    }
}

pub type BResult<T> = Result<T, BError>;
