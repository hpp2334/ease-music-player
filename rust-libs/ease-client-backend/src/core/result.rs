#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("handler not found: {0}")]
    HandlerNotFound(u32),
    #[error("other error: {0}")]
    OtherError(#[from] anyhow::Error),
    #[error("rmp encode error: {0}")]
    RmpEncodeError(#[from] rmp_serde::encode::Error),
    #[error("rmp decode error: {0}")]
    RmpDecodeError(#[from] rmp_serde::decode::Error),
}

pub type ChannelResult<T> = Result<T, ChannelError>;

pub const CHANNEL_RESULT_NIL: ChannelResult<()> = ChannelResult::<()>::Ok(());
