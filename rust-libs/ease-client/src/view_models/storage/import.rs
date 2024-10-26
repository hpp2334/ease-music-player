use std::collections::HashSet;

use ease_client_shared::{
    backends::storage::{
        ListStorageEntryChildrenResp, StorageEntry, StorageEntryLoc, StorageEntryType, StorageId,
    },
    uis::storage::{CurrentStorageImportType, CurrentStorageStateType},
};
use misty_vm::{AppBuilderContext, IToHost, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::{
        connector::Connector,
        music::lyric::MusicLyricVM,
        playlist::{create::PlaylistCreateVM, detail::PlaylistDetailVM, edit::PlaylistEditVM},
    },
};

use super::state::{AllStorageState, CurrentStorageState};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum StorageImportWidget {
    StorageItem { id: StorageId },
    StorageEntry { path: String },
    Nav { path: String },
    Import,
}

pub struct StorageImportVM {
    current: Model<CurrentStorageState>,
    store: Model<AllStorageState>,
}

fn get_entry_type(entry: &StorageEntry) -> StorageEntryType {
    const MUSIC_EXTS: [&str; 5] = [".wav", ".mp3", ".aac", ".flac", ".ogg"];
    const IMAGE_EXTS: [&str; 3] = [".jpg", ".jpeg", ".png"];
    const LYRIC_EXTS: [&str; 1] = [".lrc"];

    if entry.is_dir {
        return StorageEntryType::Folder;
    }
    let p: &str = entry.path.as_ref();

    if MUSIC_EXTS.iter().any(|ext| p.ends_with(*ext)) {
        return StorageEntryType::Music;
    }
    if IMAGE_EXTS.iter().any(|ext| p.ends_with(*ext)) {
        return StorageEntryType::Image;
    }
    if LYRIC_EXTS.iter().any(|ext| p.ends_with(*ext)) {
        return StorageEntryType::Lyric;
    }
    return StorageEntryType::Other;
}

fn can_multi_select(import_type: CurrentStorageImportType) -> bool {
    match import_type {
        CurrentStorageImportType::None |
        CurrentStorageImportType::ImportMusics { .. }
        | CurrentStorageImportType::CreatePlaylistEntries => true,
        CurrentStorageImportType::CreatePlaylistCover
        | CurrentStorageImportType::EditPlaylistCover
        | CurrentStorageImportType::CurrentMusicLyrics { .. } => false,
    }
}

fn entry_can_check(entry: &StorageEntry, import_type: CurrentStorageImportType) -> bool {
    let entry_type = get_entry_type(entry);

    match import_type {
        CurrentStorageImportType::None |
        CurrentStorageImportType::CreatePlaylistCover
        | CurrentStorageImportType::EditPlaylistCover => entry_type == StorageEntryType::Image,
        CurrentStorageImportType::CreatePlaylistEntries => {
            entry_type == StorageEntryType::Image || entry_type == StorageEntryType::Music
        }
        CurrentStorageImportType::ImportMusics {..} => entry_type == StorageEntryType::Music,
        CurrentStorageImportType::CurrentMusicLyrics { .. } => {
            entry_type == StorageEntryType::Lyric
        }
    }
}

impl StorageImportVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            store: cx.model(),
        }
    }

    fn change_storage(&self, cx: &ViewModelContext, id: StorageId) -> EaseResult<()> {
        let exist = cx.model_get(&self.store).storages.contains_key(&id);
        if exist {
            let mut state = cx.model_mut(&self.current);
            state.current_storage_id = Some(id);
            state.checked_entries_path.clear();
            state.current_path = Default::default();
        }
        Ok(())
    }

    fn select_file_entry_impl(&self, cx: &ViewModelContext, entry: StorageEntry) {
        let mut state = cx.model_mut(&self.current);
        if !entry_can_check(&entry, state.import_type) {
            return;
        }

        let can_multi_select = can_multi_select(state.import_type);
        let set: &mut HashSet<String> = &mut state.checked_entries_path;

        if can_multi_select {
            if set.contains(&entry.path) {
                set.remove(&entry.path);
            } else {
                set.insert(entry.path);
            }
        } else {
            set.clear();
            set.insert(entry.path);
        }
    }

    fn select_folder_entry_impl(
        &self,
        cx: &ViewModelContext,
        entry: StorageEntry,
    ) -> EaseResult<()> {
        let current = self.current.clone();
        let storage_id = cx.model_get(&self.current).current_storage_id;
        let storage_id = match storage_id {
            Some(storage_id) => storage_id,
            None => return Ok(()),
        };

        {
            let mut state = cx.model_mut(&self.current);
            state.state_type = CurrentStorageStateType::Loading;
            state.current_path = entry.path.clone();
            state.checked_entries_path.clear();
        }
        cx.spawn::<_, _, EaseError>(move |cx| async move {
            let connector = Connector::of(&cx);
            let res = connector
                .list_storage_entry_children(&cx, StorageEntryLoc {
                    path: entry.path,
                    storage_id,
                })
                .await?;

            let mut state = cx.model_mut(&current);
            if state.current_storage_id == Some(storage_id) {
                match res {
                    ListStorageEntryChildrenResp::Ok(vec) => {
                        state.state_type = CurrentStorageStateType::OK;
                        state.entries = vec;
                    }
                    ListStorageEntryChildrenResp::AuthenticationFailed => {
                        state.state_type = CurrentStorageStateType::AuthenticationFailed;
                        state.entries.clear();
                    }
                    ListStorageEntryChildrenResp::Timeout => {
                        state.state_type = CurrentStorageStateType::Timeout;
                        state.entries.clear();
                    }
                    ListStorageEntryChildrenResp::Unknown => {
                        state.state_type = CurrentStorageStateType::UnknownError;
                        state.entries.clear();
                    }
                }
            }

            Ok(())
        });

        Ok(())
    }

    fn select_entry(&self, cx: &ViewModelContext, path: String) -> EaseResult<()> {
        let entry = cx
            .model_get(&self.current)
            .entries
            .iter()
            .find(|m| m.path == path)
            .map(|m| m.clone());

        let entry = match entry {
            Some(entry) => entry,
            None => {
                return Ok(());
            }
        };

        if entry.is_dir {
            self.select_folder_entry_impl(cx, entry)
        } else {
            self.select_file_entry_impl(cx, entry);
            Ok(())
        }
    }

    fn handle_import(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let current_state = cx.model_get(&self.current).clone();
        let mut entries: Vec<StorageEntry> = current_state.entries.into_iter().collect();
        let storage_id = current_state.current_storage_id;
        let storage_id = match storage_id {
            Some(id) => id,
            None => return Ok(()),
        };

        match current_state.import_type {
            CurrentStorageImportType::None => {
                panic!("CurrentStorageImportType is None");
            }
            CurrentStorageImportType::ImportMusics { id } => {
                PlaylistDetailVM::of(cx).finish_import(cx, id, storage_id, entries)?;
            }
            CurrentStorageImportType::EditPlaylistCover => {
                let entry = entries.pop();
                if let Some(entry) = entry {
                    PlaylistEditVM::of(cx).finish_cover(
                        cx,
                        StorageEntryLoc {
                            storage_id,
                            path: entry.path,
                        },
                    )?;
                }
            }
            CurrentStorageImportType::CreatePlaylistEntries => {
                PlaylistCreateVM::of(cx).finish_import(cx, storage_id, entries)?;
            }
            CurrentStorageImportType::CreatePlaylistCover => {
                let entry = entries.pop();
                if let Some(entry) = entry {
                    PlaylistCreateVM::of(cx).finish_cover(
                        cx,
                        StorageEntryLoc {
                            storage_id,
                            path: entry.path,
                        },
                    )?;
                }
            }
            CurrentStorageImportType::CurrentMusicLyrics { id: _id } => {
                let entry = entries.pop();
                if let Some(entry) = entry {
                    MusicLyricVM::of(cx).handle_import_lyric(
                        cx,
                        StorageEntryLoc {
                            storage_id,
                            path: entry.path,
                        },
                    )?;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn prepare(
        &self,
        cx: &ViewModelContext,
        typ: CurrentStorageImportType,
    ) -> EaseResult<()> {
        {
            let mut state = cx.model_mut(&self.current);
            state.import_type = typ;
            state.entries.clear();
        }

        Ok(())
    }
}

impl ViewModel<Action, EaseError> for StorageImportVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => {
                match action {
                    ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                        (Widget::StroageImport(widget), WidgetActionType::Click) => match widget {
                            StorageImportWidget::StorageItem { id } => {
                                self.change_storage(cx, *id)?;
                            }
                            StorageImportWidget::StorageEntry { path } => {
                                self.select_entry(cx, path.clone())?;
                            }
                            StorageImportWidget::Nav { path } => {
                                self.select_entry(cx, path.clone())?;
                            }
                            StorageImportWidget::Import => {
                                self.handle_import(cx)?;
                            }
                        },
                        _ => {}
                    },
                    _ => {}
                }
            },
            _ => {}
        }
        Ok(())
    }
}
