use std::collections::HashSet;

use ease_client_shared::{
    backends::storage::{
        ListStorageEntryChildrenResp, Storage, StorageEntry, StorageEntryLoc, StorageEntryType,
        StorageId, StorageType,
    },
    uis::storage::{CurrentStorageImportType, CurrentStorageStateType},
};
use misty_vm::{AppBuilderContext, AsyncTasks, IToHost, Model, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    to_host::permission::PermissionService,
    view_models::{
        connector::Connector,
        main::{router::RouterVM, MainAction},
        music::lyric::MusicLyricVM,
        playlist::{create::PlaylistCreateVM, detail::PlaylistDetailVM, edit::PlaylistEditVM},
    },
    RoutesKey,
};

use super::state::{AllStorageState, CurrentStorageState};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum StorageImportWidget {
    StorageItem { id: StorageId },
    StorageEntry { path: String },
    FolderNav { path: String },
    ToggleAll,
    Import,
    Error,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum StorageImportAction {
    Reload,
    Undo,
}

pub(crate) struct StorageImportVM {
    current: Model<CurrentStorageState>,
    store: Model<AllStorageState>,
    tasks: AsyncTasks,
}

pub(crate) fn get_entry_type(entry: &StorageEntry) -> StorageEntryType {
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
        CurrentStorageImportType::None
        | CurrentStorageImportType::ImportMusics { .. }
        | CurrentStorageImportType::CreatePlaylistEntries => true,
        CurrentStorageImportType::CreatePlaylistCover
        | CurrentStorageImportType::EditPlaylistCover
        | CurrentStorageImportType::CurrentMusicLyrics { .. } => false,
    }
}

pub(crate) fn entry_can_check(entry: &StorageEntry, import_type: CurrentStorageImportType) -> bool {
    let entry_type = get_entry_type(entry);

    match import_type {
        CurrentStorageImportType::None
        | CurrentStorageImportType::CreatePlaylistCover
        | CurrentStorageImportType::EditPlaylistCover => entry_type == StorageEntryType::Image,
        CurrentStorageImportType::CreatePlaylistEntries => {
            entry_type == StorageEntryType::Image || entry_type == StorageEntryType::Music
        }
        CurrentStorageImportType::ImportMusics { .. } => entry_type == StorageEntryType::Music,
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
            tasks: Default::default(),
        }
    }

    fn change_storage(&self, cx: &ViewModelContext, id: StorageId) -> EaseResult<()> {
        let exist = cx.model_get(&self.store).storages.contains_key(&id);
        if exist {
            let mut state = cx.model_mut(&self.current);
            let store = cx.model_get(&self.store);
            let prev_storage_id = state.current_storage_id;

            if prev_storage_id != Some(id) {
                state.current_storage_id = Some(id);
                state.checked_entries_path.clear();
                state.current_path = self.get_current_path(id, &store);
                state.undo_stack.clear();
            }
        }
        self.reload(cx);
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

    fn toggle_all(&self, cx: &ViewModelContext) {
        let mut state = cx.model_mut(&self.current);

        if !state.checked_entries_path.is_empty() {
            state.checked_entries_path.clear();
        } else {
            let entries = state.entries.clone();
            for entry in entries.iter() {
                if entry_can_check(entry, state.import_type) {
                    state.checked_entries_path.insert(entry.path.clone());
                }
            }
        }
    }

    fn select_folder_entry<const PUSH_UNDO: bool>(
        &self,
        cx: &ViewModelContext,
        path: String,
    ) -> EaseResult<()> {
        let storage_id = cx.model_get(&self.current).current_storage_id;
        let _ = match storage_id {
            Some(storage_id) => storage_id,
            None => return Ok(()),
        };

        {
            let mut state = cx.model_mut(&self.current);

            if PUSH_UNDO {
                let path = state.current_path.clone();
                state.undo_stack.push(path);
            }
            state.state_type = CurrentStorageStateType::Loading;
            state.current_path = path.clone();
            state.checked_entries_path.clear();
        }
        self.reload(cx);

        Ok(())
    }

    fn reload(&self, cx: &ViewModelContext) {
        self.check_can_mutate(cx);
        let current = self.current.clone();
        let (storage_id, current_path) = {
            let m = cx.model_get(&self.current);

            (m.current_storage_id.unwrap(), m.current_path.clone())
        };
        let storage = {
            let store = cx.model_get(&self.store);
            store.storages.get(&storage_id).unwrap().clone()
        };
        if storage.typ == StorageType::Local && !PermissionService::of(cx).have_storage_permission()
        {
            let mut current = cx.model_mut(&current);
            current.state_type = CurrentStorageStateType::NeedPermission;
            return;
        }

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let connector = Connector::of(&cx);
            let res = connector
                .list_storage_entry_children(
                    &cx,
                    StorageEntryLoc {
                        path: current_path,
                        storage_id,
                    },
                )
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
            self.select_folder_entry::<true>(cx, entry.path)
        } else {
            self.select_file_entry_impl(cx, entry);
            Ok(())
        }
    }

    fn undo(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let entry = {
            let mut state = cx.model_mut(&self.current);
            let entry = state.undo_stack.pop();
            entry
        };

        if let Some(entry) = entry {
            self.select_folder_entry::<false>(cx, entry)?;
        } else {
            RouterVM::of(cx).pop(cx);
        }
        Ok(())
    }

    fn handle_import(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.check_can_mutate(cx);
        let current_state = cx.model_get(&self.current).clone();
        let mut entries: Vec<StorageEntry> = current_state.checked_entries();
        let storage_id = current_state.current_storage_id;
        let storage_id = match storage_id {
            Some(id) => id,
            None => return Ok(()),
        };

        match current_state.import_type {
            CurrentStorageImportType::None => {
                unreachable!()
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
                PlaylistCreateVM::of(cx).finish_import(cx, entries)?;
            }
            CurrentStorageImportType::CreatePlaylistCover => {
                let entry = entries.pop();
                if let Some(entry) = entry {
                    PlaylistCreateVM::of(cx).finish_cover(cx, entry)?;
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
        RouterVM::of(cx).pop(cx);

        Ok(())
    }

    fn on_error_click(&self, cx: &ViewModelContext) {
        let state = cx.model_get(&self.current);
        if state.state_type == CurrentStorageStateType::NeedPermission {
            PermissionService::of(cx).request_storage_permission();
        } else {
            self.reload(cx);
        }
    }

    pub(crate) fn prepare(
        &self,
        cx: &ViewModelContext,
        typ: CurrentStorageImportType,
    ) -> EaseResult<()> {
        {
            let store = cx.model_get(&self.store);
            let mut state = cx.model_mut(&self.current);
            state.import_type = typ;
            state.state_type = CurrentStorageStateType::Loading;
            state.entries.clear();
            state.checked_entries_path.clear();
            state.undo_stack.clear();

            if state.current_storage_id.is_none() {
                state.current_storage_id = Some(store.storage_ids[0]);
            }
            let id = state.current_storage_id.unwrap();
            state.current_path = self.get_current_path(id, &store);
        }
        self.reload(cx);
        RouterVM::of(cx).navigate(cx, RoutesKey::ImportMusics);

        Ok(())
    }

    fn get_current_path(&self, id: StorageId, store: &AllStorageState) -> String {
        let storage = store.storages.get(&id);
        if let Some(storage) = storage {
            if storage.typ == StorageType::Local {
                return store.local_storage_path.to_string();
            }
        }
        return "/".to_string();
    }

    fn check_can_mutate(&self, cx: &ViewModelContext) {
        let (storage_id, import_type) = {
            let m = cx.model_get(&self.current);

            (m.current_storage_id, m.import_type)
        };

        if storage_id.is_none() {
            panic!("storage_id is None");
        }
        if import_type == CurrentStorageImportType::None {
            panic!("import_type is None");
        }
    }
}

impl ViewModel for StorageImportVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::StorageImport(widget), WidgetActionType::Click) => match widget {
                        StorageImportWidget::StorageItem { id } => {
                            self.change_storage(cx, *id)?;
                        }
                        StorageImportWidget::StorageEntry { path } => {
                            self.select_entry(cx, path.clone())?;
                        }
                        StorageImportWidget::FolderNav { path } => {
                            self.select_folder_entry::<true>(cx, path.clone())?;
                        }
                        StorageImportWidget::Import => {
                            self.handle_import(cx)?;
                        }
                        StorageImportWidget::ToggleAll => self.toggle_all(cx),
                        StorageImportWidget::Error => self.on_error_click(cx),
                    },
                    _ => {}
                },
                ViewAction::StorageImport(action) => match action {
                    StorageImportAction::Reload => self.reload(cx),
                    StorageImportAction::Undo => self.undo(cx)?,
                },
                ViewAction::Main(action) => match action {
                    MainAction::PermissionChanged => self.reload(cx),
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
