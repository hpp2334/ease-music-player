use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use ease_client_shared::{
    backends::{
        music::MusicId,
        storage::{
            ArgUpsertStorage, GetStorageMsg, ListStorageEntryChildrenMsg, ListStorageMsg,
            RemoveStorageMsg, Storage, StorageConnectionTestResult, StorageEntry, StorageEntryLoc,
            StorageEntryType, StorageId, TestStorageMsg, UpsertStorageMsg,
        },
    },
    uis::storage::{CurrentStorageImportType, CurrentStorageStateType},
};
use misty_vm::{
    async_task::MistyAsyncTaskTrait,
    client::{AsMistyClientHandle, MistyClientHandle},
    services::MistyServiceTrait,
    states::MistyStateTrait,
    MistyAsyncTask, MistyState,
};

use crate::modules::{
    app::service::get_backend,
    error::{EaseError, EaseResult, EASE_RESULT_NIL},
    music::service::{get_current_music_id, import_selected_lyric_in_music},
    playlist::service::{
        import_selected_cover_in_create_playlist, import_selected_cover_in_edit_playlist,
        import_selected_entries_in_create_playlist, import_selected_entries_to_current_playlist,
        reload_all_playlists_state,
    },
    timer::to_host::TimerService,
};

#[derive(Default, MistyState)]
pub struct StoragesState {
    pub storages: Vec<Storage>,
}

#[derive(Default, Clone, MistyState)]
pub struct CurrentStorageState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub entries: Vec<StorageEntry>,
    pub checked_entries_path: HashSet<String>,
    pub current_storage_id: Option<StorageId>,
    pub current_path: String,
    pub attach_music_id: Option<MusicId>,
}

#[derive(Default, Clone, MistyState)]
pub struct StoragesRecordState {
    pub last_locate_path: HashMap<StorageId, String>,
}

#[derive(Default, Clone, MistyState)]
pub struct EditStorageState {
    pub is_create: bool,
    pub title: String,
    pub info: ArgUpsertStorage,
    pub music_count: u32,
    pub playlist_count: u32,
    pub test: StorageConnectionTestResult,
    pub update_signal: u16,
}

#[derive(Debug, MistyAsyncTask)]
struct LocateEntryOnceAsyncTask;

#[derive(Debug, MistyAsyncTask)]
struct GeneralAsyncTask;

pub fn get_entry_type(entry: &StorageEntry) -> StorageEntryType {
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

fn get_storage_id(app: MistyClientHandle) -> EaseResult<StorageId> {
    CurrentStorageState::map(app, |state| state.current_storage_id)
        .ok_or(EaseError::CurrentMusicNone)
}

fn can_multi_select(import_type: CurrentStorageImportType) -> bool {
    match import_type {
        CurrentStorageImportType::Musics | CurrentStorageImportType::CreatePlaylistEntries => true,
        CurrentStorageImportType::CreatePlaylistCover
        | CurrentStorageImportType::EditPlaylistCover
        | CurrentStorageImportType::CurrentMusicLyrics => false,
    }
}

pub(super) fn entry_can_check(entry: &StorageEntry, import_type: CurrentStorageImportType) -> bool {
    let entry_type = get_entry_type(entry);

    match import_type {
        CurrentStorageImportType::CreatePlaylistCover
        | CurrentStorageImportType::EditPlaylistCover => entry_type == StorageEntryType::Image,
        CurrentStorageImportType::CreatePlaylistEntries => {
            entry_type == StorageEntryType::Image || entry_type == StorageEntryType::Music
        }
        CurrentStorageImportType::Musics => entry_type == StorageEntryType::Music,
        CurrentStorageImportType::CurrentMusicLyrics => entry_type == StorageEntryType::Lyric,
    }
}

pub(super) fn resolve_storage_name(info: &Storage) -> String {
    info.alias.clone().unwrap_or(info.addr.clone())
}

pub fn locate_entry_impl(
    client: MistyClientHandle,
    storage_id: StorageId,
    path: String,
    candidate_path: String,
) -> EaseResult<()> {
    let backend = get_backend(client);

    let current_path = path.to_string();
    CurrentStorageState::update(client, |state| {
        state.state_type = CurrentStorageStateType::Loading;
        state.current_path = current_path;
        state.checked_entries_path.clear();
    });

    let path = path.to_string();
    LocateEntryOnceAsyncTask::spawn_once(client, move |ctx| async move {
        let mut list = backend
            .send::<ListStorageEntryChildrenMsg>(StorageEntryLoc { path, storage_id })
            .await?;
        if list.is_error() {
            list = backend
                .send::<ListStorageEntryChildrenMsg>(StorageEntryLoc {
                    path: candidate_path,
                    storage_id,
                })
                .await?;
        }

        ctx.schedule(move |client| {
            let should_not_handle = CurrentStorageState::map(client, |state| {
                state.state_type != CurrentStorageStateType::Loading
                    || state.current_storage_id.is_none()
            });
            if should_not_handle {
                return EASE_RESULT_NIL;
            }

            CurrentStorageState::update(client, |state| {
                match list {
                    ease_client_shared::backends::storage::ListStorageEntryChildrenResp::Ok(list) => {
                        state.state_type = CurrentStorageStateType::OK;
                        state.entries = list;
                    },
                    ease_client_shared::backends::storage::ListStorageEntryChildrenResp::AuthenticationFailed => {
                        state.state_type = CurrentStorageStateType::AuthenticationFailed;
                    },
                    ease_client_shared::backends::storage::ListStorageEntryChildrenResp::Timeout => {
                        state.state_type = CurrentStorageStateType::Timeout;
                    },
                    ease_client_shared::backends::storage::ListStorageEntryChildrenResp::Unknown => {
                        state.state_type = CurrentStorageStateType::UnknownError;
                    },
                }

            });
            return EASE_RESULT_NIL;
        });
        return EASE_RESULT_NIL;
    });
    return EASE_RESULT_NIL;
}

fn refresh_storage_in_import(client: MistyClientHandle, storage_id: StorageId) -> EaseResult<()> {
    const DEFAULT_URL: &str = "/";

    let last_path = StoragesRecordState::map(client, |state| {
        state.last_locate_path.get(&storage_id).map(|p| p.clone())
    })
    .unwrap_or(DEFAULT_URL.to_string());

    CurrentStorageState::update(client, |state| {
        state.entries.clear();
        state.current_storage_id = Some(storage_id.clone());
    });

    locate_entry_impl(client, storage_id, last_path, DEFAULT_URL.to_string())?;
    return EASE_RESULT_NIL;
}

pub fn locate_entry(client: MistyClientHandle, path: String) -> EaseResult<()> {
    let storage_id = get_storage_id(client)?;

    StoragesRecordState::update(client, |state| {
        state.last_locate_path.insert(storage_id, path.clone());
    });

    locate_entry_impl(client, storage_id, path, Default::default())?;
    EASE_RESULT_NIL
}

fn leave_storage(client: MistyClientHandle) {
    CurrentStorageState::update(client, |state| {
        state.current_storage_id = None;
        state.checked_entries_path.clear();
    });
}

fn get_checked_entries(client: MistyClientHandle) -> Vec<StorageEntry> {
    let entries = CurrentStorageState::map(client, |state| {
        let ret: Vec<StorageEntry> = state
            .entries
            .clone()
            .into_iter()
            .filter(|e| state.checked_entries_path.contains(&e.path))
            .collect();
        return ret;
    });
    return entries;
}

pub fn prepare_edit_storage(
    cx: MistyClientHandle,
    storage_id: Option<StorageId>,
) -> EaseResult<()> {
    let backend = get_backend(cx);
    GeneralAsyncTask::spawn(cx, move |cx| async move {
        let storage = if let Some(storage_id) = storage_id {
            backend.send::<GetStorageMsg>(storage_id).await?
        } else {
            None
        };

        cx.schedule(move |client| {
            if let Some(storage) = storage {
                EditStorageState::update(client, |state| {
                    state.is_create = false;
                    state.title = resolve_storage_name(&storage);
                    state.info = ArgUpsertStorage {
                        id: Some(storage.id),
                        addr: storage.addr.clone(),
                        alias: storage.alias.clone(),
                        username: storage.username.clone(),
                        password: storage.password.clone(),
                        is_anonymous: storage.is_anonymous,
                        typ: storage.typ,
                    };
                    state.update_signal += 1;
                    state.test = StorageConnectionTestResult::None;
                })
            } else {
                EditStorageState::update(client, |state| {
                    state.is_create = true;
                    state.info = Default::default();
                    state.test = StorageConnectionTestResult::None;
                    state.update_signal += 1;
                })
            }
            return EASE_RESULT_NIL;
        });

        return EASE_RESULT_NIL;
    });
    Ok(())
}

pub fn upsert_storage(app: MistyClientHandle, arg: ArgUpsertStorage) -> EaseResult<()> {
    let backend = get_backend(app);
    GeneralAsyncTask::spawn(app.handle(), |cx| async move {
        backend.send::<UpsertStorageMsg>(arg).await?;

        cx.schedule(reload_storages_state);
        return EASE_RESULT_NIL;
    });

    return EASE_RESULT_NIL;
}

pub fn reload_storages_state(cx: MistyClientHandle) -> EaseResult<()> {
    let backend = get_backend(cx);
    GeneralAsyncTask::spawn(cx, |cx| async move {
        let storages = backend.send::<ListStorageMsg>(()).await?;

        cx.schedule(|cx| {
            StoragesState::update(cx, |state| {
                state.storages = storages;
            });
            return EASE_RESULT_NIL;
        });

        return EASE_RESULT_NIL;
    });
    return EASE_RESULT_NIL;
}

pub fn toggle_all_checked_entries(app: MistyClientHandle) {
    CurrentStorageState::update(app, |state| {
        let set = &mut state.checked_entries_path;
        let import_type = state.import_type;
        if !set.is_empty() {
            set.clear();
        } else {
            set.clear();
            state.entries.iter().for_each(|e| {
                if entry_can_check(e, import_type) {
                    set.insert(e.path.clone());
                }
            });
        }
    });
}

pub fn select_entry(app: MistyClientHandle, path: String) {
    CurrentStorageState::update(app, |state| {
        let entry = state.entries.iter().find(|e| e.path == path);
        if entry.is_none() || !entry_can_check(entry.unwrap(), state.import_type) {
            return;
        }

        let set: &mut HashSet<String> = &mut state.checked_entries_path;
        let can_multi_select = can_multi_select(state.import_type);

        if can_multi_select {
            if set.contains(&path) {
                set.remove(&path);
            } else {
                set.insert(path);
            }
        } else {
            set.clear();
            set.insert(path);
        }
    });
}

pub fn select_storage_in_import(client: MistyClientHandle, id: StorageId) -> EaseResult<()> {
    refresh_storage_in_import(client, id)?;
    Ok(())
}

pub fn enter_storages_to_import(
    client: MistyClientHandle,
    import_type: CurrentStorageImportType,
) -> EaseResult<()> {
    let id = StoragesState::map(client, |state| {
        state.storages.iter().next().map(|v| v.id.clone())
    });
    if id.is_none() {
        return Ok(());
    }

    let music_id = get_current_music_id(client);
    if import_type == CurrentStorageImportType::CurrentMusicLyrics && music_id.is_none() {
        return Err(EaseError::OtherError("attach music id is none".to_string()));
    }

    CurrentStorageState::update(client, |state| {
        state.import_type = import_type;
        state.attach_music_id = music_id;
    });
    refresh_storage_in_import(client, id.unwrap())?;
    return Ok(());
}

pub fn refresh_current_storage_in_import(client: MistyClientHandle) -> EaseResult<()> {
    let id = CurrentStorageState::map(client, |state| state.current_storage_id.clone());
    if id.is_none() {
        return Ok(());
    }
    refresh_storage_in_import(client, id.unwrap())?;
    return Ok(());
}

pub(super) fn remove_storage(cx: MistyClientHandle, storage_id: StorageId) -> EaseResult<()> {
    let backend = get_backend(cx);
    GeneralAsyncTask::spawn(cx, move |cx| async move {
        backend.send::<RemoveStorageMsg>(storage_id).await?;

        cx.schedule(reload_storages_state);
        cx.schedule(reload_all_playlists_state);

        return EASE_RESULT_NIL;
    });
    Ok(())
}

pub fn finish_select_entries_in_import(client: MistyClientHandle) -> EaseResult<()> {
    let (storage_id, import_type, music_id) = CurrentStorageState::map(client, |state| {
        (
            state.current_storage_id.unwrap(),
            state.import_type,
            state.attach_music_id,
        )
    });
    if import_type == CurrentStorageImportType::CurrentMusicLyrics && music_id.is_none() {
        return Err(EaseError::OtherError("attach music id is none".to_string()));
    }

    let mut entries = get_checked_entries(client);
    leave_storage(client);

    match import_type {
        CurrentStorageImportType::Musics => {
            import_selected_entries_to_current_playlist(client, storage_id, entries)?;
        }
        CurrentStorageImportType::EditPlaylistCover => {
            let entry = entries.pop().unwrap();
            import_selected_cover_in_edit_playlist(client, storage_id, entry);
        }
        CurrentStorageImportType::CreatePlaylistEntries => {
            import_selected_entries_in_create_playlist(client, storage_id, entries)?;
        }
        CurrentStorageImportType::CreatePlaylistCover => {
            let entry = entries.pop().unwrap();
            import_selected_cover_in_create_playlist(client, storage_id, entry);
        }
        CurrentStorageImportType::CurrentMusicLyrics => {
            let music_id = music_id.unwrap();
            let entry = entries.pop().unwrap();
            import_selected_lyric_in_music(client, music_id, storage_id, entry)?;
        }
    }
    Ok(())
}

#[derive(Debug, MistyAsyncTask)]
struct TestConnectionAsyncTask;

pub(super) fn edit_storage_test_connection(
    cx: MistyClientHandle,
    arg: ArgUpsertStorage,
) -> EaseResult<()> {
    EditStorageState::update(cx, |state| {
        state.test = StorageConnectionTestResult::Testing;
    });
    let backend = get_backend(cx);

    TestConnectionAsyncTask::spawn_once(cx, |ctx| async move {
        let test_result = backend.send::<TestStorageMsg>(arg.clone()).await?;

        ctx.schedule(move |handle| {
            EditStorageState::update(handle, |state| {
                state.test = test_result;
            });
            EASE_RESULT_NIL
        });

        TimerService::of_async(&ctx)
            .wait(Duration::from_secs(3))
            .await;
        ctx.schedule(|handle| {
            EditStorageState::update(handle, |state| {
                state.test = StorageConnectionTestResult::None;
            });
            EASE_RESULT_NIL
        });

        EASE_RESULT_NIL
    });

    Ok(())
}
