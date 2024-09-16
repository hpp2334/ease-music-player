#[derive(Debug, thiserror::Error)]
pub enum BError {
    #[error("database error: {0}")]
    DatabaseError(#[from] ease_database::Error),
}

pub type BResult<T> = Result<T, BError>;
