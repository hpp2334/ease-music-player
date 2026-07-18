use std::sync::Arc;
use ease_client_tokio::tokio_runtime;

use ease_client_schema::PlayMode;

use crate::{
    error::BResult,
    services::{get_preference_playmode, save_preference_playmode},
    Backend,
};

#[uniffi::export]
pub async fn cts_save_preference_playmode(cx: Arc<Backend>, arg: PlayMode) -> BResult<()> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        save_preference_playmode(cx, arg).await?;
        Ok(())
    }).await.unwrap()
}

#[uniffi::export]
pub async fn cts_get_preference_playmode(cx: Arc<Backend>) -> BResult<PlayMode> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        get_preference_playmode(cx).await
    }).await.unwrap()
}
