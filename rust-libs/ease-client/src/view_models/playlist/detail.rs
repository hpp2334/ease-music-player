
use ease_client_shared::{backends::{
    music::MusicId, playlist::{ArgAddMusicsToPlaylist, PlaylistId}, storage::{StorageEntry, StorageEntryLoc, StorageId}
}, uis::storage::CurrentStorageImportType};
use misty_vm::{AppBuilderContext, ViewModel, ViewModelContext};

use crate::{
    actions::{Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::{connector::Connector, music::{common::MusicCommonVM, control::MusicControlVM}, storage::import::StorageImportVM},
};

use super::{common::PlaylistCommonVM, edit::PlaylistEditVM};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlaylistDetailWidget {
    Remove,
    Edit,
    Music { id: MusicId },
    RemoveMusic { id: MusicId },
    Import,
}

pub struct PlaylistDetailVM {}

impl PlaylistDetailVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {}
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
                let name = v.path.clone().split("/").last().expect("path is empty").to_string();
                let loc = StorageEntryLoc {
                    storage_id,
                    path: v.path,
                };

                (loc, name)
            })
            .collect();
            
        cx.spawn::<_, _, EaseError>(move |cx| async move {
            Connector::of(&cx).add_musics_to_playlist(ArgAddMusicsToPlaylist {
                id: playlist_id,
                entries,
            }).await?;
            Ok(())
        });

        Ok(())
    }
}

impl ViewModel<Action, EaseError> for PlaylistDetailVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::Widget(action) => match (&action.widget, &action.typ) {
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
                        MusicControlVM::of(cx).play(cx, *id)?;
                    }
                    PlaylistDetailWidget::RemoveMusic { id } => {
                        MusicCommonVM::of(cx).remove(cx, *id)?;
                    }
                    PlaylistDetailWidget::Import => {
                        let current = PlaylistCommonVM::of(cx).get_current(cx)?;
                        if let Some(current) = current {
                            StorageImportVM::of(cx).prepare(cx, CurrentStorageImportType::ImportMusics { id: current.id(), })?;
                        }
                    }
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
