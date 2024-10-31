use ease_client_shared::{
    backends::{
        music::MusicId,
        playlist::{ArgAddMusicsToPlaylist, PlaylistId},
        storage::{StorageEntry, StorageEntryLoc, StorageId},
    },
    uis::storage::CurrentStorageImportType,
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::{
        connector::Connector,
        music::{common::MusicCommonVM, control::MusicControlVM},
        storage::import::StorageImportVM,
    },
};

use super::{common::PlaylistCommonVM, edit::PlaylistEditVM, state::CurrentPlaylistState};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlaylistDetailWidget {
    Remove,
    Edit,
    Music { id: MusicId },
    RemoveMusic { id: MusicId },
    Import,
    PlayAll,
}

pub(crate) struct PlaylistDetailVM {
    current: Model<CurrentPlaylistState>,
    tasks: AsyncTasks,
}

fn trim_extension_name(name: impl ToString) -> String {
    name.to_string()
        .rsplit_once('.')
        .map_or(name.to_string(), |(base, _)| base.to_string())
}

impl PlaylistDetailVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            tasks: Default::default(),
        }
    }

    fn play_all(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if let Some(playlist) = cx.model_get(&self.current).playlist.as_ref() {
            if let Some(music_id) = playlist.musics.first().map(|m| m.id()) {
                MusicControlVM::of(cx).request_play(cx, music_id)?;
            }
        }
        Ok(())
    }

    pub(crate) fn prepare_current(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        let current = self.current.clone();
        {
            let mut state = cx.model_mut(&current);
            state.playlist.take();
        }

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let playlist = Connector::of(&cx).get_playlist(&cx, id).await?;

            let mut state = cx.model_mut(&current);
            state.playlist = playlist;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn finish_import(
        &self,
        cx: &ViewModelContext,
        playlist_id: PlaylistId,
        storage_id: StorageId,
        entries: Vec<StorageEntry>,
    ) -> EaseResult<()> {
        let entries = entries
            .into_iter()
            .map(|v| {
                let name = trim_extension_name(v.name);
                let loc = StorageEntryLoc {
                    storage_id,
                    path: v.path,
                };

                (loc, name)
            })
            .collect();

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .add_musics_to_playlist(
                    &cx,
                    ArgAddMusicsToPlaylist {
                        id: playlist_id,
                        entries,
                    },
                )
                .await?;
            Ok(())
        });

        Ok(())
    }
}

impl ViewModel for PlaylistDetailVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::PlaylistDetail(action), WidgetActionType::Click) => match action {
                        PlaylistDetailWidget::Remove => {
                            PlaylistCommonVM::of(cx).remove_current(cx)?;
                        }
                        PlaylistDetailWidget::Edit => {
                            let playlist = PlaylistCommonVM::of(cx)
                                .get_current(cx)?
                                .expect("edit playlist but current playlist is None");
                            PlaylistEditVM::of(cx).prepare_edit(cx, &playlist)?;
                        }
                        PlaylistDetailWidget::Music { id } => {
                            MusicControlVM::of(cx).request_play(cx, *id)?;
                        }
                        PlaylistDetailWidget::RemoveMusic { id } => {
                            let playlist_id = cx
                                .model_get(&self.current)
                                .playlist
                                .as_ref()
                                .map(|p| p.id());
                            if let Some(playlist_id) = playlist_id {
                                MusicCommonVM::of(cx).remove(cx, *id, playlist_id)?;
                            }
                        }
                        PlaylistDetailWidget::Import => {
                            let current = PlaylistCommonVM::of(cx).get_current(cx)?.unwrap();
                            StorageImportVM::of(cx).prepare(
                                cx,
                                CurrentStorageImportType::ImportMusics { id: current.id() },
                            )?;
                        }
                        PlaylistDetailWidget::PlayAll => self.play_all(cx)?,
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
