
#[derive(Debug, thiserror::Error)]
pub enum BError {
    #[error("database error: {0}")]
    DatabaseError(#[from] ease_database::Error),
    #[error("no such message error: code = {0}")]
    NoSuchMessage(u32),
}

pub type BResult<T> = Result<T, BError>;
