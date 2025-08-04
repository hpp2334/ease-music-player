use crate::{ctx::BackendContext, error::BResult, objects::PlayMode};

pub(crate) fn save_preference_playmode(cx: &BackendContext, arg: PlayMode) -> BResult<()> {
    let mut data = cx.database_server().load_preference()?;
    data.playmode = arg;
    cx.database_server().save_preference(data)?;
    Ok(())
}
