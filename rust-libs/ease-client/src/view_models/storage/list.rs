use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::EaseError,
};
use ease_client_shared::backends::storage::StorageId;
use misty_vm::{AppBuilderContext, ViewModel, ViewModelContext};

use super::upsert::StorageUpsertVM;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum StorageListWidget {
    Create,
    Item { id: StorageId },
}

pub(crate) struct StorageListVM {}

impl StorageListVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        StorageListVM {}
    }
}

impl ViewModel for StorageListVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::StorageList(action), WidgetActionType::Click) => match action {
                        StorageListWidget::Create => {
                            StorageUpsertVM::of(&cx).prepare_create(&cx)?;
                        }
                        StorageListWidget::Item { id } => {
                            StorageUpsertVM::of(&cx).prepare_edit(cx, *id)?;
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
