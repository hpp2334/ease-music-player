use misty_vm::{AppBuilderContext, IToHost, ViewModel, ViewModelContext};

use crate::{error::EaseResult, Action, AndroidRoutesKey, DesktopRoutesKey, EaseError, RouterService, ViewAction, Widget, WidgetActionType};

use super::RouterVM;

pub(crate) struct DesktopSidebarVM {}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum DesktopSidebarWidget {
    Playlists,
    Settings,
}


impl DesktopSidebarVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }
}

impl ViewModel for DesktopSidebarVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::DesktopSidebar(action), WidgetActionType::Click) => match action {
                        DesktopSidebarWidget::Playlists =>{
                            RouterVM::of(cx).navigate_desktop(cx, DesktopRoutesKey::Home);
                        },
                        DesktopSidebarWidget::Settings =>{
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
