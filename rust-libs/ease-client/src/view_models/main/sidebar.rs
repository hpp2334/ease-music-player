use misty_vm::{AppBuilderContext, IToHost, ViewModel, ViewModelContext};

use crate::{error::EaseResult, Action, AndroidRoutesKey, DesktopRoutesKey, EaseError, RouterService, ViewAction, Widget, WidgetActionType};

use super::RouterVM;

pub(crate) struct SidebarVM {}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum SidebarWidget {
    Playlists,
    Settings,
}


impl SidebarVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }

    pub(crate) fn navigate(&self, cx: &ViewModelContext, key: AndroidRoutesKey) {
        RouterService::of(cx).navigate(key);
    }

    pub(crate) fn pop(&self, cx: &ViewModelContext) {
        RouterService::of(cx).pop();
    }
}

impl ViewModel for SidebarVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::Sidebar(action), WidgetActionType::Click) => match action {
                        SidebarWidget::Playlists =>{
                            RouterVM::of(cx).navigate_desktop(cx, DesktopRoutesKey::Home);
                        },
                        SidebarWidget::Settings =>{
                            RouterVM::of(cx).navigate_desktop(cx, DesktopRoutesKey::Setting);
                        },
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
