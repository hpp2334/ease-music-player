use misty_vm::{AppBuilderContext, IToHost, ViewModel, ViewModelContext};

use crate::{error::EaseResult, Action, AndroidRoutesKey, DesktopRoutesKey, EaseError, RouterService, ViewAction};

pub(crate) struct RouterVM {}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum RouterAction {
    Pop,
}

impl RouterVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }

    pub(crate) fn navigate(&self, cx: &ViewModelContext, key: AndroidRoutesKey) {
        RouterService::of(cx).navigate(key);
    }

    pub(crate) fn navigate_desktop(&self, cx: &ViewModelContext, key: DesktopRoutesKey) {
        RouterService::of(cx).navigate_desktop(key);
    }

    pub(crate) fn pop(&self, cx: &ViewModelContext) {
        RouterService::of(cx).pop();
    }
}

impl ViewModel for RouterVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Router(action) => match action {
                    RouterAction::Pop => {
                        self.pop(cx);
                    }
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
