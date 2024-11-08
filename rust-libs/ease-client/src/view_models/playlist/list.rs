use ease_client_shared::backends::playlist::PlaylistId;
use misty_vm::{AppBuilderContext, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::EaseError,
};

use super::{create::PlaylistCreateVM, detail::PlaylistDetailVM};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlaylistListWidget {
    Add,
    Item { id: PlaylistId },
}

pub(crate) struct PlaylistListVM {}

impl PlaylistListVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }
}

impl ViewModel for PlaylistListVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::PlaylistList(action), WidgetActionType::Click) => match action {
                        PlaylistListWidget::Add => {
                            PlaylistCreateVM::of(cx).prepare(cx)?;
                        }
                        PlaylistListWidget::Item { id } => {
                            PlaylistDetailVM::of(cx).prepare_current(cx, *id)?;
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
