#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("handler not found: {0}")]
    HandlerNotFound(u32),
    #[error("other error: {0}")]
    OtherError(#[from] anyhow::Error),
}

pub type ChannelResult<T> = Result<T, ChannelError>;

pub const CHANNEL_RESULT_NIL: ChannelResult<()> = ChannelResult::<()>::Ok(());
