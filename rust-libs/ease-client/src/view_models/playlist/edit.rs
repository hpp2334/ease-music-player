use ease_client_shared::{
    backends::{
        playlist::{ArgUpdatePlaylist, Playlist},
        storage::StorageEntryLoc,
    },
    uis::storage::CurrentStorageImportType,
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::{connector::Connector, storage::import::StorageImportVM},
};

use super::state::EditPlaylistState;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlaylistEditWidget {
    Name,
    ClearCover,
    Cover,
    FinishEdit,
}

pub(crate) struct PlaylistEditVM {
    form: Model<EditPlaylistState>,
    tasks: AsyncTasks,
}

impl PlaylistEditVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            form: cx.model(),
            tasks: Default::default(),
        }
    }

    pub(crate) fn prepare_edit(
        &self,
        cx: &ViewModelContext,
        playlist: &Playlist,
    ) -> EaseResult<()> {
        let mut form = cx.model_mut(&self.form);
        form.id = Some(playlist.id());
        form.cover = playlist.cover().as_ref().map(|v| v.clone());
        form.playlist_name = playlist.title().to_string();
        Ok(())
    }

    fn prepare_cover(&self, cx: &ViewModelContext) -> EaseResult<()> {
        StorageImportVM::of(cx).prepare(cx, CurrentStorageImportType::EditPlaylistCover)?;
        Ok(())
    }

    pub(crate) fn finish_cover(
        &self,
        cx: &ViewModelContext,
        loc: StorageEntryLoc,
    ) -> EaseResult<()> {
        let mut form = cx.model_mut(&self.form);
        form.cover = Some(loc);
        Ok(())
    }

    fn finish_edit(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let arg = {
            let form = cx.model_get(&self.form);
            ArgUpdatePlaylist {
                id: form.id.expect("finish edit but playlist id is None"),
                title: form.playlist_name.to_string(),
                cover: form.cover.clone(),
            }
        };
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).update_playlist(&cx, arg).await?;
            Ok(())
        });
        Ok(())
    }
}

impl ViewModel for PlaylistEditVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::PlaylistEdit(action), WidgetActionType::Click) => match action {
                        PlaylistEditWidget::Cover => StorageImportVM::of(cx)
                            .prepare(cx, CurrentStorageImportType::EditPlaylistCover)?,
                        PlaylistEditWidget::ClearCover => {
                            let mut form = cx.model_mut(&self.form);
                            form.cover = None;
                        }
                        PlaylistEditWidget::FinishEdit => {
                            self.finish_edit(cx)?;
                        }
                        _ => {}
                    },
                    (Widget::PlaylistEdit(action), WidgetActionType::ChangeText { text }) => {
                        match action {
                            PlaylistEditWidget::Name => {
                                let mut form = cx.model_mut(&self.form);
                                form.playlist_name = text.clone();
                            }
                            PlaylistEditWidget::Cover => {
                                self.prepare_cover(cx)?;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
