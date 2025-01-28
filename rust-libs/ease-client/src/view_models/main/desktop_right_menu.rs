use misty_vm::{AppBuilderContext, IToHost, Model, ViewModel, ViewModelContext};

use crate::{error::EaseResult, Action, AndroidRoutesKey, DesktopRoutesKey, EaseError, RouterService, StorageListWidget, ViewAction, Widget, WidgetActionType};

use super::{state::{RightMenuState, RightMenuValue}, RouterVM};

pub(crate) struct DesktopRightMenuVM {
    state: Model<RightMenuState>
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum DesktopRightMenuWidget {
    Mask
}

impl DesktopRightMenuVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            state: cx.model(),
        }
    }

    fn clear(&self, cx: &ViewModelContext) {
        let mut state = cx.model_mut(&self.state);
        state.visible = false;
        state.value = RightMenuValue::None;
    }
}

impl ViewModel for DesktopRightMenuVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::DesktopRightMenu(action), WidgetActionType::Click | WidgetActionType::RightClick { .. }) => match action {
                        DesktopRightMenuWidget::Mask => {
                            self.clear(cx);
                        },
                    },
                    (Widget::StorageList(action), WidgetActionType::Click) => match action {
                        StorageListWidget::Item { .. } => {
                            self.clear(cx);
                        },
                        _ => {}
                    },
                    (Widget::StorageList(action), WidgetActionType::RightClick { x, y }) => match action {
                        StorageListWidget::Item { id } => {
                            let mut state = cx.model_mut(&self.state);
                            state.visible = true;
                            state.x = *x;
                            state.y = *y;
                            state.value = RightMenuValue::Storage(*id);
                        },
                        _ => {}
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
