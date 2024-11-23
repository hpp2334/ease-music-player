use misty_vm::{AppBuilderContext, IToHost, ViewModel, ViewModelContext};

use crate::{error::EaseResult, Action, EaseError, RouterService, RoutesKey, ViewAction};

pub(crate) struct RouterVM {}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum RouterAction {
    Pop,
}

impl RouterVM {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }

    pub(crate) fn navigate(&self, cx: &ViewModelContext, key: RoutesKey) {
        RouterService::of(cx).naviagate(key);
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
