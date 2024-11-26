use std::time::Duration;

use ease_client_shared::backends::{
    connector::ConnectorAction,
    generated::{DisableTimeToPauseMsg, EnableTimeToPauseMsg},
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::connector::Connector,
};

use super::{common::MusicCommonVM, state::TimeToPauseState};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum TimeToPauseWidget {
    Delete,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum TimeToPauseAction {
    Finish { hour: u8, minute: u8, second: u8 },
    CloseModal,
}

pub(crate) struct TimeToPauseVM {
    timer: Model<TimeToPauseState>,
    tasks: AsyncTasks,
}

impl TimeToPauseVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            timer: cx.model(),
            tasks: Default::default(),
        }
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.update_timer(cx)
    }

    pub(crate) fn pause(&self, cx: &ViewModelContext) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .request::<DisableTimeToPauseMsg>(&cx, ())
                .await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn open(&self, cx: &ViewModelContext) {
        self.update_modal_open(cx, true);
    }

    fn update_modal_open(&self, cx: &ViewModelContext, value: bool) {
        let mut form = cx.model_mut(&self.timer);
        form.modal_open = value;
    }

    fn start_timer(
        &self,
        cx: &ViewModelContext,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> EaseResult<()> {
        let delay = Duration::from_secs(hour as u64 * 3600)
            + Duration::from_secs(minute as u64 * 60)
            + Duration::from_secs(second as u64);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .request::<EnableTimeToPauseMsg>(&cx, delay)
                .await?;
            Ok(())
        });
        Ok(())
    }

    fn update_timer(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.timer);
        let s_time = cx.get_time().min(state.expired_time);
        state.left = state.expired_time - s_time;

        if !state.left.is_zero() {
            drop(state);
            MusicCommonVM::of(cx).schedule_tick(cx)?;
        }
        Ok(())
    }
}

impl ViewModel for TimeToPauseVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::TimeToPause(action), WidgetActionType::Click) => match action {
                        TimeToPauseWidget::Delete => {
                            self.pause(cx)?;
                        }
                    },
                    _ => {}
                },
                ViewAction::TimeToPause(action) => match action {
                    TimeToPauseAction::Finish {
                        hour,
                        minute,
                        second,
                    } => {
                        self.start_timer(cx, *hour, *minute, *second)?;
                    }
                    TimeToPauseAction::CloseModal => {
                        self.update_modal_open(cx, false);
                    }
                },
                _ => {}
            },
            Action::Connector(action) => match action {
                ConnectorAction::TimeToPause(info) => {
                    {
                        let mut state = cx.model_mut(&self.timer);
                        state.enabled = info.enabled;
                        state.expired_time = info.expired;
                        state.left = info.left;
                    }
                    MusicCommonVM::of(cx).schedule_tick(cx)?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
