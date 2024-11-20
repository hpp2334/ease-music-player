use ease_client_shared::backends::playlist::PlaylistId;
use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::EaseError,
};

use super::{create::PlaylistCreateVM, detail::PlaylistDetailVM, state::AllPlaylistState};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlaylistListWidget {
    Add,
    Item { id: PlaylistId },
}

pub(crate) struct PlaylistListVM {
    store: Model<AllPlaylistState>,
}

impl PlaylistListVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self { store: cx.model() }
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
