
use ease_client_shared::{
    backends::preference::PreferenceData,
    uis::preference::PlayMode,
};

use crate::{
    ctx::BackendContext,
    error::BResult,
    services::app::{load_preference_data, save_preference_data},
};

pub(crate) async fn cr_get_preference(cx: BackendContext, _arg: ()) -> BResult<PreferenceData> {
    Ok(load_preference_data(&cx))
}

pub(crate) async fn cu_update_preference_playmode(
    cx: BackendContext,
    arg: PlayMode,
) -> BResult<()> {
    let mut data = load_preference_data(&cx);
    data.playmode = arg;
    save_preference_data(&cx, data);
    Ok(())
}
