use std::time::Duration;

use ease_client_shared::backends::music::MusicId;
use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    view_models::connector::Connector,
};

use super::{
    lyric::MusicLyricVM,
    state::{CurrentMusicState, TimeToPauseState},
    time_to_pause::TimeToPauseVM,
};

#[derive(Debug, uniffi::Enum)]
pub enum MusicCommonAction {
    Tick,
}

pub struct MusicCommonVM {
    current: Model<CurrentMusicState>,
    time_to_pause: Model<TimeToPauseState>,
}

impl MusicCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            time_to_pause: cx.model(),
        }
    }

    pub(crate) fn remove(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(move |cx| async move {
            let connector = Connector::of(&cx);
            connector.remove_music(id).await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn remove_current(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if let Some(current_id) = cx.model_get(&self.current).id {
            self.remove(cx, current_id)
        } else {
            Ok(())
        }
    }

    pub(crate) fn schedule_tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(move |cx| async move {
            cx.sleep(Duration::from_secs(1)).await;
            this.tick(&cx)?;
            Ok(())
        });
        Ok(())
    }

    fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let is_playing = cx.model_get(&self.current).playing;
        let time_to_pause_enabled = cx.model_get(&self.time_to_pause).enabled;

        if is_playing {
            MusicLyricVM::of(cx).tick_lyric_index(cx)?;
        }
        if time_to_pause_enabled {
            TimeToPauseVM::of(cx).tick(cx)?;
        }

        if is_playing || time_to_pause_enabled {
            self.schedule_tick(&cx)?;
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
