
use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
};

use super::{
    common::MusicCommonVM,
    state::CurrentMusicState,
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum MusicDetailWidget {
    Remove,
}

pub struct MusicDetailVM {
    current: Model<CurrentMusicState>,
}

impl MusicDetailVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
        }
    }
}

impl ViewModel<Action, EaseError> for MusicDetailVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::Widget(action) => match (&action.widget, &action.typ) {
                (Widget::MusicDetail(action), WidgetActionType::Click) => match action {
                    MusicDetailWidget::Remove => {
                        MusicCommonVM::of(cx).remove_current(cx)?;
                    }
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
