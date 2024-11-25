use std::collections::HashSet;

use ease_client_shared::backends::{
    generated::CreatePlaylistMsg,
    playlist::{ArgCreatePlaylist, CreatePlaylistMode},
    storage::{CurrentStorageImportType, StorageEntry, StorageEntryType},
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    utils::common::{decode_component_or_origin, trim_extension_name},
    view_models::{
        connector::Connector,
        storage::import::{get_entry_type, StorageImportVM},
    },
};

use super::state::CreatePlaylistState;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlaylistCreateWidget {
    Tab { value: CreatePlaylistMode },
    Name,
    ClearCover,
    Cover,
    Import,
    FinishCreate,
    Cancel,
    Reset,
    CloseModal,
}

pub(crate) struct PlaylistCreateVM {
    form: Model<CreatePlaylistState>,
    tasks: AsyncTasks,
}

fn build_recommend_playlist_names(entries: &Vec<StorageEntry>) -> Vec<String> {
    let mut recommend_playlist_names: HashSet<String> = Default::default();
    for entry in entries.iter() {
        let split: Vec<&str> = entry.path.split("/").collect();
        for i in 0..(split.len() - 1) {
            let p = split[i];
            if !p.is_empty() {
                recommend_playlist_names.insert(decode_component_or_origin(p.to_string()));
            }
        }
    }

    let mut recommend_playlist_names: Vec<String> = recommend_playlist_names.into_iter().collect();
    recommend_playlist_names.sort_by(|a, b| b.len().cmp(&a.len()));
    recommend_playlist_names
}

impl PlaylistCreateVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            form: cx.model(),
            tasks: Default::default(),
        }
    }

    pub(crate) fn prepare(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.reset(cx)?;
        self.update_modal_open(cx, true);
        Ok(())
    }

    fn update_modal_open(&self, cx: &ViewModelContext, value: bool) {
        let mut form = cx.model_mut(&self.form);
        form.modal_open = value;
    }

    fn reset(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut form = cx.model_mut(&self.form);
        form.mode = CreatePlaylistMode::Full;
        form.cover = None;
        form.playlist_name = Default::default();
        form.entries.clear();
        form.recommend_playlist_names.clear();
        Ok(())
    }

    fn prepare_cover(&self, cx: &ViewModelContext) -> EaseResult<()> {
        StorageImportVM::of(cx).prepare(cx, CurrentStorageImportType::CreatePlaylistCover)?;
        Ok(())
    }

    pub(crate) fn finish_import(
        &self,
        cx: &ViewModelContext,
        entries: Vec<StorageEntry>,
    ) -> EaseResult<()> {
        let cover = entries
            .iter()
            .filter(|v| get_entry_type(v) == StorageEntryType::Image)
            .map(|v| v.clone())
            .next();
        let entries: Vec<StorageEntry> = entries
            .into_iter()
            .filter(|v| get_entry_type(v) == StorageEntryType::Music)
            .collect();

        let mut form = cx.model_mut(&self.form);
        form.recommend_playlist_names = build_recommend_playlist_names(&entries);
        if form.playlist_name.is_empty() && !form.recommend_playlist_names.is_empty() {
            form.playlist_name = form.recommend_playlist_names[0].clone();
        }
        form.cover = cover.map(|v| v.loc());
        form.entries = entries;

        Ok(())
    }

    pub(crate) fn finish_cover(
        &self,
        cx: &ViewModelContext,
        entry: StorageEntry,
    ) -> EaseResult<()> {
        let mut form = cx.model_mut(&self.form);
        form.cover = Some(entry.loc());
        Ok(())
    }

    fn finish_create(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let arg = {
            let form = cx.model_get(&self.form);
            ArgCreatePlaylist {
                title: form.playlist_name.to_string(),
                cover: form.cover.clone(),
                entries: form
                    .entries
                    .iter()
                    .map(|e| (e.clone(), trim_extension_name(&e.name)))
                    .collect(),
            }
        };
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .request::<CreatePlaylistMsg>(&cx, arg)
                .await?;
            Ok(())
        });
        Ok(())
    }
}

impl ViewModel for PlaylistCreateVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::PlaylistCreate(action), WidgetActionType::Click) => match action {
                        PlaylistCreateWidget::Tab { value } => {
                            let mut form = cx.model_mut(&self.form);
                            form.mode = *value;
                        }
                        PlaylistCreateWidget::Name => {
                            unimplemented!()
                        }
                        PlaylistCreateWidget::ClearCover => {
                            let mut form = cx.model_mut(&self.form);
                            form.cover = None;
                        }
                        PlaylistCreateWidget::Cover => {
                            self.prepare_cover(cx)?;
                        }
                        PlaylistCreateWidget::Import => {
                            StorageImportVM::of(cx)
                                .prepare(cx, CurrentStorageImportType::CreatePlaylistEntries)?;
                        }
                        PlaylistCreateWidget::FinishCreate => {
                            self.finish_create(cx)?;
                        }
                        PlaylistCreateWidget::Cancel => {
                            self.reset(cx)?;
                        }
                        PlaylistCreateWidget::Reset => {
                            self.reset(cx)?;
                        }
                        PlaylistCreateWidget::CloseModal => {
                            self.update_modal_open(cx, false);
                        }
                    },
                    (Widget::PlaylistCreate(action), WidgetActionType::ChangeText { text }) => {
                        match action {
                            PlaylistCreateWidget::Name => {
                                let mut form = cx.model_mut(&self.form);
                                form.playlist_name = text.to_string();
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
