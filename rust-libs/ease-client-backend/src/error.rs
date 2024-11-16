use ease_client_shared::backends::generated::Code;

#[derive(Debug, thiserror::Error)]
pub enum BError {
    #[error("database error: {0:?}")]
    DatabaseError(#[from] ease_database::Error),
    #[error("remote storage error: {0:?}")]
    RemoteStorageError(#[from] ease_remote_storage::StorageBackendError),
    #[error("no such message error: code = {0:?}")]
    NoSuchMessage(Code),
}

pub type BResult<T> = Result<T, BError>;
