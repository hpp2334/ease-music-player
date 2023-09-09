use misty_vm::{
    client::{AsReadonlyMistyClientHandle, MistyClientHandle},
    states::MistyStateTrait,
};

use crate::modules::{
    app::service::{load_preference_data, save_preference_data},
    error::{EaseResult, EASE_RESULT_NIL},
};

use super::{PlayMode, PreferenceState};

pub fn update_playmode(app: MistyClientHandle, playmode: PlayMode) -> EaseResult<()> {
    let _state = PreferenceState::update(app, |state| {
        state.play_mode = playmode;
        state.clone()
    });
    save_preference_data(app);
    return EASE_RESULT_NIL;
}

pub fn get_playmode(app: MistyClientHandle) -> PlayMode {
    PreferenceState::map(app, |state| state.play_mode.clone())
}

pub fn init_preference_state(app: MistyClientHandle) -> EaseResult<()> {
    let data = load_preference_data(app);
    PreferenceState::update(app, |state| {
        *state = data;
    });
    Ok(())
}

pub fn preference_state_to_data<'a>(app: impl AsReadonlyMistyClientHandle<'a>) -> PreferenceState {
    PreferenceState::map(app, Clone::clone)
}
