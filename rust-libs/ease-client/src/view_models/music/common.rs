use std::time::Duration;

use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
};

use super::{lyric::MusicLyricVM, state::CurrentMusicState};

#[derive(Debug, uniffi::Enum)]
pub enum MusicCommonAction {
    Tick,
}

pub struct MusicCommonVM {
    current: Model<CurrentMusicState>,
}

impl MusicCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
        }
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let this = Self::of(cx);
        MusicLyricVM::of(cx).tick_lyric_index(&cx)?;

        let is_playing = cx.model_get(&self.current).playing;
        if is_playing {
            cx.spawn::<_, _, EaseError>(move |cx| async move {
                cx.sleep(Duration::from_secs(1)).await;
                this.tick(&cx)?;
                Ok(())
            });
        }
        Ok(())
    }
}

impl ViewModel<Action, EaseError> for MusicCommonVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::MusicCommon(action) => match action {
                MusicCommonAction::Tick => {
                    self.tick(cx)?;
                }
            },
            _ => {}
        }
        Ok(())
    }
}
