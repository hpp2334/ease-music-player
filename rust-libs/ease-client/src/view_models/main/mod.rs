use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};
use router::RouterVM;
use state::{RootRouteSubKey, RouterState};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    RoutesKey,
};

use super::{
    connector::Connector, music::time_to_pause::TimeToPauseVM, storage::common::StorageCommonVM,
};

pub mod router;
pub mod state;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum MainBodyWidget {
    Tab { key: RootRouteSubKey },
    TimeToPause,
    MiniPlayer,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum MainAction {
    PermissionChanged,
}

pub(crate) struct MainBodyVM {
    router_sub: Model<RouterState>,
}

impl MainBodyVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            router_sub: cx.model(),
        }
    }
}

impl ViewModel for MainBodyVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::MainBody(action), WidgetActionType::Click) => match action {
                        MainBodyWidget::Tab { key } => {
                            let mut state = cx.model_mut(&self.router_sub);
                            state.subkey = *key;
                        }
                        MainBodyWidget::TimeToPause => {
                            TimeToPauseVM::of(cx).open(cx);
                        }
                        MainBodyWidget::MiniPlayer => {
                            RouterVM::of(cx).navigate(cx, RoutesKey::MusicPlayer);
                        }
                    },
                    _ => {}
                },
                _ => {}
            },
            Action::Init(arg) => {
                Connector::of(cx).init(cx, arg.clone())?;
                StorageCommonVM::of(cx).update_local_storage_path(cx, arg.storage_path.clone());
            }
            _ => {}
        }
        Ok(())
    }
}
