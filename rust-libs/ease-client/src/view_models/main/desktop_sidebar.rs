use ease_client_shared::backends::playlist::PlaylistId;
use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    error::EaseResult,
    view_models::playlist::{detail::PlaylistDetailVM, state::AllPlaylistState},
    Action, DesktopRoutesKey, EaseError, ViewAction, Widget, WidgetActionType,
};

use super::RouterVM;

pub(crate) struct DesktopSidebarVM {
    store: Model<AllPlaylistState>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum DesktopSidebarWidget {
    Playlists,
    Settings,
    Playlist { id: PlaylistId },
}

impl DesktopSidebarVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self { store: cx.model() }
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
                        DesktopSidebarWidget::Playlists => {
                            RouterVM::of(cx).navigate_desktop(cx, DesktopRoutesKey::Home);
                        }
                        DesktopSidebarWidget::Settings => {
                            RouterVM::of(cx).navigate_desktop(cx, DesktopRoutesKey::Setting);
                        }
                        DesktopSidebarWidget::Playlist { id } => {
                            let playlist_abstr = cx
                                .model_get(&self.store)
                                .playlists
                                .iter()
                                .find(|v| v.id() == *id)
                                .cloned();
                            if let Some(playlist_abstr) = playlist_abstr {
                                PlaylistDetailVM::of(cx).prepare_current(cx, playlist_abstr)?;
                            }
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
