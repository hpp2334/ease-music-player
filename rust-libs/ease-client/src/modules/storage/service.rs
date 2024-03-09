use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use ease_remote_storage::{Backend, BuildWebdavArg, Entry, LocalBackend, StreamFile, Webdav};
use misty_vm::{
    async_task::MistyAsyncTaskTrait,
    client::{AsReadonlyMistyClientHandle, MistyClientHandle},
    services::MistyServiceTrait,
    states::MistyStateTrait,
    MistyAsyncTask, MistyState,
};

use crate::{
    modules::{
        app::service::get_has_local_storage_permission,
        error::{EaseError, EaseResult, EASE_RESULT_NIL},
        music::service::{get_current_music_id, import_selected_lyric_in_music},
        playlist::service::{
            get_playlist_musics_by_storage, import_selected_cover_in_create_playlist,
            import_selected_cover_in_edit_playlist, import_selected_entries_in_create_playlist,
            import_selected_entries_to_current_playlist, remove_musics_in_playlists_by_storage,
        },
        storage::repository::db_upsert_storage,
        timer::to_host::TimerService,
        MusicId, PlaylistId,
    },
    utils::cmp_name_smartly,
};

use super::{
    repository::{
        db_init_local_storage_db_if_not_exist, db_load_storage, db_load_storage_infos,
        db_remove_storage,
    },
    typ::*,
    StorageType,
};

#[derive(Default, MistyState)]
pub struct StoragesState {
    pub storage_ids: HashSet<StorageId>,
    pub storage_infos: Vec<StorageInfo>,
    pub is_init: bool,
}

#[derive(Default, MistyState)]
pub struct StorageBackendStaticState {
    backend_map: HashMap<StorageId, Arc<dyn Backend + Send + Sync>>,
}

#[derive(Default, Clone, MistyState)]
pub struct CurrentStorageState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub entries: Vec<Entry>,
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
    pub test: StorageConnectionTestResult,
    pub tuple_maps: HashMap<PlaylistId, Vec<MusicId>>,
    pub update_signal: u16,
}

#[derive(Debug, MistyAsyncTask)]
struct LocateEntryOnceAsyncTask;

pub fn get_entry_type(entry: &Entry) -> StorageEntryType {
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

pub(super) fn entry_can_check(entry: &Entry, import_type: CurrentStorageImportType) -> bool {
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

pub fn locate_entry_impl(
    client: MistyClientHandle,
    storage_id: StorageId,
    path: String,
    candidate_path: String,
) -> EaseResult<()> {
    let backend: Arc<dyn Backend + Send + Sync> = get_storage_backend(client, storage_id.clone())?;

    let current_path = path.to_string();
    CurrentStorageState::update(client, |state| {
        state.state_type = CurrentStorageStateType::Loading;
        state.current_path = current_path;
        state.checked_entries_path.clear();
    });

    let path = path.to_string();
    LocateEntryOnceAsyncTask::spawn_once(client, move |ctx| async move {
        let mut list = backend.list(&path).await;
        if list.is_err() {
            list = backend.list(&candidate_path).await;
        }

        ctx.schedule(move |client| {
            let should_not_handle = CurrentStorageState::map(client, |state| {
                state.state_type != CurrentStorageStateType::Loading
                    || state.current_storage_id.is_none()
            });
            if should_not_handle {
                return EASE_RESULT_NIL;
            }

            let is_local = StoragesState::map(client, |state| {
                state
                    .storage_infos
                    .iter()
                    .any(|v| v.typ == StorageType::Local && v.id == storage_id)
            });
            let has_local_permission = get_has_local_storage_permission(client);
            if is_local && !has_local_permission {
                CurrentStorageState::update(client, |state| {
                    state.state_type = CurrentStorageStateType::NeedPermission;
                });
                return EASE_RESULT_NIL;
            }
            if let Err(e) = list {
                tracing::error!("{:?}", e);
                if e.is_unauthorized() {
                    CurrentStorageState::update(client, |state| {
                        state.state_type = CurrentStorageStateType::AuthenticationFailed;
                    });
                } else if e.is_timeout() {
                    CurrentStorageState::update(client, |state: &mut CurrentStorageState| {
                        state.state_type = CurrentStorageStateType::Timeout;
                    });
                } else {
                    CurrentStorageState::update(client, |state| {
                        state.state_type = CurrentStorageStateType::UnknownError;
                    });
                }
                return EASE_RESULT_NIL;
            }

            let list = list.unwrap();
            CurrentStorageState::update(client, |state| {
                state.state_type = CurrentStorageStateType::OK;
                state.entries = list;
            });
            return EASE_RESULT_NIL;
        });
        return EASE_RESULT_NIL;
    });
    return EASE_RESULT_NIL;
}

fn refresh_storage_in_import(client: MistyClientHandle, storage_id: StorageId) -> EaseResult<()> {
    let backend: Arc<dyn Backend + Send + Sync> = get_storage_backend(client, storage_id.clone())?;

    let last_path = StoragesRecordState::map(client, |state| {
        state.last_locate_path.get(&storage_id).map(|p| p.clone())
    })
    .unwrap_or(backend.default_url());

    CurrentStorageState::update(client, |state| {
        state.entries.clear();
        state.current_storage_id = Some(storage_id.clone());
    });

    locate_entry_impl(client, storage_id, last_path, backend.default_url())?;
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

fn get_checked_entries(client: MistyClientHandle) -> Vec<Entry> {
    let entries = CurrentStorageState::map(client, |state| {
        let ret: Vec<Entry> = state
            .entries
            .clone()
            .into_iter()
            .filter(|e| state.checked_entries_path.contains(&e.path))
            .collect();
        return ret;
    });
    return entries;
}

fn clear_storage_backend_cache(app: MistyClientHandle, id: StorageId) {
    StorageBackendStaticState::update(app, |state| {
        state.backend_map.remove(&id);
    });
}

pub fn prepare_edit_storage(
    client: MistyClientHandle,
    storage_id: Option<StorageId>,
) -> EaseResult<()> {
    if let Some(storage_id) = storage_id {
        let storage = db_load_storage(client, storage_id.clone())?;
        let tuple_maps = get_playlist_musics_by_storage(client, storage_id)?;
        EditStorageState::update(client, |state| {
            state.is_create = false;
            state.title = storage
                .alias()
                .clone()
                .map(|s| s.to_string())
                .unwrap_or(storage.addr().to_string());
            state.info = ArgUpsertStorage {
                id: Some(storage_id),
                addr: storage.addr().to_string(),
                alias: storage.alias().clone().map(|s| s.to_string()),
                username: storage.username().to_owned(),
                password: storage.password().to_owned(),
                is_anonymous: storage.is_anonymous(),
                typ: storage.typ(),
            };
            state.tuple_maps = tuple_maps;
            state.update_signal += 1;
            state.test = StorageConnectionTestResult::None;
        })
    } else {
        EditStorageState::update(client, |state| {
            state.is_create = true;
            state.info = Default::default();
            state.test = StorageConnectionTestResult::None;
        })
    }
    Ok(())
}

pub fn upsert_storage(app: MistyClientHandle, arg: ArgUpsertStorage) -> EaseResult<()> {
    tracing::info!("storage with addr {} is upsert", arg.addr);
    let id = arg.id.clone();
    db_upsert_storage(app, arg)?;
    if let Some(id) = id {
        clear_storage_backend_cache(app, id);
    }
    update_storages_state(app, true)?;
    return EASE_RESULT_NIL;
}

pub fn update_storages_state(app: MistyClientHandle, force: bool) -> EaseResult<()> {
    if !force {
        let is_init = StoragesState::map(app, |state: &StoragesState| state.is_init);
        if is_init {
            return EASE_RESULT_NIL;
        }
    }

    let map = db_load_storage_infos(app)?;
    let storage_ids = map.iter().map(|v| v.0.clone()).collect();
    let mut storage_infos: Vec<StorageInfo> = map.into_iter().map(|v| v.1).collect();
    storage_infos.sort_by(|lhs, rhs| {
        if lhs.typ == StorageType::Local {
            return Ordering::Greater;
        }
        if rhs.typ == StorageType::Local {
            return Ordering::Less;
        }
        return cmp_name_smartly(&lhs.name, &rhs.name);
    });

    StoragesState::update(app, move |state: &mut StoragesState| {
        state.storage_ids = storage_ids;
        state.storage_infos = storage_infos;
        state.is_init = true;
    });
    return EASE_RESULT_NIL;
}

fn get_storage_backend<'a>(
    app: impl AsReadonlyMistyClientHandle<'a>,
    storage_id: StorageId,
) -> EaseResult<Arc<dyn Backend + Send + Sync>> {
    let cache_backend = StorageBackendStaticState::map(app.clone(), |backend_map_state| {
        let cache_backend = backend_map_state
            .backend_map
            .get(&storage_id)
            .map(|v| v.clone());
        cache_backend
    });
    if cache_backend.is_some() {
        return Ok(cache_backend.unwrap());
    }

    let storage = db_load_storage(app, storage_id)?;

    let connect_timeout = Duration::from_secs(5);

    let ret: Arc<dyn Backend + Send + Sync + 'static> = match storage.typ() {
        StorageType::Local => Arc::new(LocalBackend::new()),
        StorageType::Webdav => {
            let arg = BuildWebdavArg {
                addr: storage.addr().to_string(),
                username: storage.username().to_string(),
                password: storage.password().to_string(),
                is_anonymous: storage.is_anonymous(),
                connect_timeout,
            };
            Arc::new(Webdav::new(arg))
        }
        StorageType::Ftp => {
            unimplemented!()
        }
    };

    let cloned_ret = ret.clone();
    app.readonly_handle()
        .schedule(move |app| -> EaseResult<()> {
            StorageBackendStaticState::update(app, |backend_map_state| {
                backend_map_state.backend_map.insert(storage_id, cloned_ret);
            });
            Ok(())
        });
    return Ok(ret);
}

pub async fn load_storage_entry_data(
    app: impl AsReadonlyMistyClientHandle<'_>,
    storage_id: StorageId,
    path: String,
) -> EaseResult<StreamFile> {
    let backend = get_storage_backend(app, storage_id)?;
    let data = backend.get(&path).await?;
    Ok(data)
}

fn get_storage_backend_by_upsert_arg(
    arg: ArgUpsertStorage,
) -> EaseResult<Arc<dyn Backend + Send + Sync>> {
    let connect_timeout = Duration::from_secs(5);

    let ret: Arc<dyn Backend + Send + Sync + 'static> = match arg.typ {
        StorageType::Local => Arc::new(LocalBackend::new()),
        StorageType::Webdav => {
            let arg = BuildWebdavArg {
                addr: arg.addr,
                username: arg.username,
                password: arg.password,
                is_anonymous: arg.is_anonymous,
                connect_timeout,
            };
            Arc::new(Webdav::new(arg))
        }
        StorageType::Ftp => {
            unimplemented!()
        }
    };
    return Ok(ret);
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

pub fn init_local_storage_db_if_not_exist(client: MistyClientHandle) -> EaseResult<()> {
    db_init_local_storage_db_if_not_exist(client)?;
    Ok(())
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
        state.storage_infos.iter().next().map(|v| v.id.clone())
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

pub(super) fn remove_storage(client: MistyClientHandle, storage_id: StorageId) -> EaseResult<()> {
    remove_musics_in_playlists_by_storage(client, storage_id)?;
    db_remove_storage(client, storage_id)?;
    update_storages_state(client, true)?;
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
    client: MistyClientHandle,
    arg: ArgUpsertStorage,
) -> EaseResult<()> {
    EditStorageState::update(client, |state| {
        state.test = StorageConnectionTestResult::Testing;
    });
    let backend = get_storage_backend_by_upsert_arg(arg)?;

    TestConnectionAsyncTask::spawn_once(client, |ctx| async move {
        let result = backend.list("/").await;

        let test_result = if let Err(e) = result {
            if e.is_timeout() {
                StorageConnectionTestResult::Timeout
            } else if e.is_unauthorized() {
                StorageConnectionTestResult::Unauthorized
            } else {
                StorageConnectionTestResult::OtherError
            }
        } else {
            StorageConnectionTestResult::Success
        };

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
