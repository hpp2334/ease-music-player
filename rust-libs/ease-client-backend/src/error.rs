use ease_client_schema::{MusicId, PlaylistId};
use ease_order_key::OrderKeyError;

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum BError {
    #[error("remote storage error: {0:?}")]
    RemoteStorageError(#[from] ease_remote_storage::StorageBackendError),
    #[error("failed to load asset: {0:?}")]
    AssetLoadFail(String),
    #[error("asset not found")]
    AssetNotFound,
    #[error("playlist not found")]
    PlaylistNotFound(PlaylistId),
    #[error("music not found")]
    MusicNotFound(MusicId),
    #[error("database error: {0:?}")]
    DbError(#[from] sea_orm::DbErr),
    #[error("io error: {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("json error: {0:?}")]
    JsonError(#[from] serde_json::Error),
    #[error(transparent)]
    OrderKeyError(#[from] OrderKeyError),
    #[error("custom: {message}")]
    CustomError { message: String },
    #[error(transparent)]
    AnyHowError(#[from] anyhow::Error),
}

pub type BResult<T> = Result<T, BError>;
