#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum EaseError {
    #[error("current music is none")]
    CurrentMusicNone,
    #[error("current playlist is none")]
    CurrentPlaylistNone,
    #[error("edit playlist is none")]
    CurrentStorageNone,
    #[error("edit playlist is none")]
    EditPlaylistNone,
    #[error("serde error")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("other error: {0}")]
    OtherError(String),
    #[error("client destroyed")]
    ClientDestroyed,
}

pub type EaseResult<T> = Result<T, EaseError>;

pub const EASE_RESULT_NIL: EaseResult<()> = EaseResult::<()>::Ok(());
