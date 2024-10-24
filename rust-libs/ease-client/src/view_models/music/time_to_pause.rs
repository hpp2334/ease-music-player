use std::time::Duration;

use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
};

use super::{
    common::MusicCommonVM,
    state::{CurrentMusicState, TimeToPauseState},
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum TimeToPauseWidget {
    Delete,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum TimeToPauseAction {
    Finish { hour: u8, minute: u8 },
}

pub struct TimeToPauseVM {
    timer: Model<TimeToPauseState>,
}

impl TimeToPauseVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self { timer: cx.model() }
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.update_timer(cx)
    }

    pub(crate) fn pause(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut edit = cx.model_mut(&self.timer);
        edit.enabled = false;
        Ok(())
    }

    fn start_timer(&self, cx: &ViewModelContext, hour: u8, minute: u8) -> EaseResult<()> {
        {
            let mut state = cx.model_mut(&self.timer);
            let s_time = cx.get_time();
            let t_time = s_time
                + Duration::from_secs(hour as u64 * 3600)
                + Duration::from_secs(minute as u64 * 60);
            state.expired_time = t_time;
            state.enabled = true;
        }
        self.update_timer(cx)?;
        Ok(())
    }

    fn update_timer(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.timer);
        let s_time = cx.get_time().max(state.expired_time);
        state.left = state.expired_time - s_time;

        if state.left.is_zero() {
            state.enabled = false;
        } else {
            MusicCommonVM::of(cx).schedule_tick(cx)?;
        }
        Ok(())
    }
}

impl ViewModel<Action, EaseError> for TimeToPauseVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::Widget(action) => match (&action.widget, &action.typ) {
                (Widget::TimeToPause(action), WidgetActionType::Click) => match action {
                    TimeToPauseWidget::Delete => {
                        self.pause(cx)?;
                    }
                },
                _ => {}
            },
            Action::TimeToPause(action) => match action {
                TimeToPauseAction::Finish { hour, minute } => {
                    self.start_timer(cx, *hour, *minute)?;
                }
            },
            _ => {}
        }
        Ok(())
    }
}
