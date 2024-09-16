use ease_client_shared::{
    backends::preference::{GetPreferenceMsg, PreferenceData, UpdatePreferenceMsg},
    uis::preference::PlayMode,
};
use misty_vm::{
    async_task::MistyAsyncTaskTrait, client::MistyClientHandle, states::MistyStateTrait,
    MistyAsyncTask,
};

use crate::modules::{
    app::service::get_backend,
    error::{EaseResult, EASE_RESULT_NIL},
};

use super::PreferenceState;

#[derive(MistyAsyncTask)]
struct GeneralAsyncTask;

pub fn update_playmode(cx: MistyClientHandle, playmode: PlayMode) -> EaseResult<()> {
    let backend = get_backend(cx);
    let data = PreferenceData { playmode };
    let cloned_data = data.clone();

    GeneralAsyncTask::spawn(cx, |_cx| async move {
        backend.send::<UpdatePreferenceMsg>(data).await?;
        return EASE_RESULT_NIL;
    });
    PreferenceState::update(cx, |state| {
        state.data = cloned_data;
    });
    return EASE_RESULT_NIL;
}

pub fn get_playmode(app: MistyClientHandle) -> PlayMode {
    PreferenceState::map(app, |state| state.data.playmode.clone())
}

pub fn reload_preference_state(cx: MistyClientHandle) -> EaseResult<()> {
    let backend = get_backend(cx);
    GeneralAsyncTask::spawn(cx, |cx| async move {
        let data = backend.send::<GetPreferenceMsg>(()).await?;

        cx.schedule(move |cx| {
            PreferenceState::update(cx, |state| {
                state.data = data;
            });
            return EASE_RESULT_NIL;
        });
        return EASE_RESULT_NIL;
    });

    Ok(())
}
