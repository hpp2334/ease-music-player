use std::sync::Arc;

use ease_client_shared::backends::player::PlayMode;

use crate::{ctx::BackendContext, error::BResult};

pub(crate) fn save_preference_playmode(cx: &BackendContext, arg: PlayMode) -> BResult<()> {
    let mut data = cx.database_server().load_preference()?;
    data.playmode = arg;
    cx.database_server().save_preference(data)?;
    Ok(())
}
