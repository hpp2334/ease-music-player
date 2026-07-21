use std::sync::Arc;
use ease_client_tokio::tokio_runtime;

use ease_client_schema::PlayMode;

use crate::{
    error::BResult,
    services::{get_preference_playmode, save_preference_playmode},
    Backend,
};

/// Synchronously save the play-mode preference (the `cts_` prefix marks
/// this as a sync controller per AGENTS.md). The underlying service fn is
/// async, so we drive it on the shared tokio runtime via `block_on`.
#[uniffi::export]
pub fn cts_save_preference_playmode(cx: Arc<Backend>, arg: PlayMode) -> BResult<()> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move {
        save_preference_playmode(&cx, arg).await?;
        Ok(())
    })
}

/// Synchronously read the play-mode preference. See
/// [`cts_save_preference_playmode`] for the sync/async reasoning.
#[uniffi::export]
pub fn cts_get_preference_playmode(cx: Arc<Backend>) -> BResult<PlayMode> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { get_preference_playmode(&cx).await })
}
