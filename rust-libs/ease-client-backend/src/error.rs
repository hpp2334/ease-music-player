use ease_client_shared::backends::generated::Code;

#[derive(Debug, thiserror::Error)]
pub enum BError {
    #[error("database error: {0:?}")]
    DatabaseError(#[from] ease_database::Error),
    #[error("remote storage error: {0:?}")]
    RemoteStorageError(#[from] ease_remote_storage::StorageBackendError),
    #[error("no such message error: code = {0:?}")]
    NoSuchMessage(Code),
    #[error("redb error: {0:?}")]
    RedbError(#[from] redb::Error),
    #[error("redb transaction error: {0:?}")]
    RedbTransactionError(#[from] redb::TransactionError),
    #[error("redb table error: {0:?}")]
    RedbTableError(#[from] redb::TableError),
    #[error("redb storage error: {0:?}")]
    RedbStorageError(#[from] redb::StorageError),
    #[error("redb commit error: {0:?}")]
    RedbCommitError(#[from] redb::CommitError),
}

pub type BResult<T> = Result<T, BError>;
