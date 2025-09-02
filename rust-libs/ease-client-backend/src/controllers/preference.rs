use std::sync::Arc;

use ease_client_schema::PlayMode;

use crate::{
    error::BResult,
    services::{get_preference_playmode, save_preference_playmode},
    Backend,
};

#[uniffi::export]
pub fn cts_save_preference_playmode(cx: Arc<Backend>, arg: PlayMode) -> BResult<()> {
    let cx = cx.get_context();
    save_preference_playmode(cx, arg)?;
    Ok(())
}

#[uniffi::export]
pub fn cts_get_preference_playmode(cx: Arc<Backend>) -> BResult<PlayMode> {
    let cx = cx.get_context();
    get_preference_playmode(cx)
}
