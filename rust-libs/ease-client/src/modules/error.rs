use axum::{response::IntoResponse, response::Response};
use ease_client_music_player::LrcParseError;
use ease_remote_storage::{BackendError, StatusCode};

use super::StorageId;

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum EaseError {
    #[error("server error")]
    ServerError(#[from] axum::Error),
    #[error("current music is none")]
    CurrentMusicNone,
    #[error("current playlist is none")]
    CurrentPlaylistNone,
    #[error("edit playlist is none")]
    CurrentStorageNone,
    #[error("edit playlist is none")]
    EditPlaylistNone,
    #[error("current storage is none")]
    BackendNone(StorageId),
    #[error("backend error")]
    BackendError(#[from] BackendError),
    #[error("serde error")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("lyric parse error: {0}")]
    LyricParseError(#[from] LrcParseError),
    #[error("database error: {0}")]
    DatabaseError(#[from] ease_database::Error),
    #[error("other error: {0}")]
    OtherError(String),
    #[error("client destroyed")]
    ClientDestroyed,
}

impl IntoResponse for EaseError {
    fn into_response(self) -> Response {
        let body = self.to_string();

        // its often easiest to implement `IntoResponse` by calling other implementations
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

pub type EaseResult<T> = Result<T, EaseError>;

pub const EASE_RESULT_NIL: EaseResult<()> = EaseResult::<()>::Ok(());
