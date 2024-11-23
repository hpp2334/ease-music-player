#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum AndroidFfiError {
    #[error("custom error: {0:?}")]
    Custom(String),
}
