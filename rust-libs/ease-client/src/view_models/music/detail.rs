use misty_vm::{AppBuilderContext, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
};

use super::common::MusicCommonVM;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum MusicDetailWidget {
    Remove,
}

pub struct MusicDetailVM {}

impl MusicDetailVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }
}

impl ViewModel<Action, EaseError> for MusicDetailVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::MusicDetail(action), WidgetActionType::Click) => match action {
                        MusicDetailWidget::Remove => {
                            MusicCommonVM::of(cx).remove_current(cx)?;
                        }
                    },
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
