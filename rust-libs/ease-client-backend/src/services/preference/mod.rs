use ease_client_shared::backends::player::PlayMode;

use crate::ctx::BackendContext;

use super::app::{load_preference_data, save_preference_data};

pub(crate) fn save_preference_playmode(cx: &BackendContext, arg: PlayMode) {
    let mut data = load_preference_data(&cx);
    data.playmode = arg;
    save_preference_data(&cx, data);
}
