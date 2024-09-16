use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use ease_client_shared::{
    backends::{
        music::MusicId,
        music_duration::MusicDuration,
        playlist::{
            AddMusicsToPlaylistMsg, ArgAddMusicsToPlaylist, GetAllPlaylistAbstractsMsg,
            GetPlaylistMsg, Playlist, PlaylistAbstract, PlaylistId, PlaylistMeta,
        },
        storage::{StorageEntry, StorageEntryType, StorageId},
    },
    uis::{playlist::CreatePlaylistMode, storage::CurrentStorageImportType},
};
use misty_vm::{
    async_task::MistyAsyncTaskTrait, client::MistyClientHandle, resources::MistyResourceHandle,
    services::MistyServiceTrait, states::MistyStateTrait, MistyAsyncTask, MistyState,
};

use crate::modules::{
    app::service::get_backend,
    error::{EaseError, EaseResult, EASE_RESULT_NIL},
    music::service::{clear_current_music_state_if_invalid, play_music},
    storage::service::{enter_storages_to_import, get_entry_type},
    timer::to_host::TimerService,
};

#[derive(Default, MistyState)]
pub struct AllPlaylistState {
    pub list: Vec<PlaylistAbstract>,
}

#[derive(Default, MistyState)]
pub struct CurrentPlaylistState {
    pub playlist: Option<Playlist>,
}

#[derive(Default, Clone, MistyState)]
pub struct EditPlaylistState {
    pub id: Option<PlaylistId>,
    pub picture: Option<MistyResourceHandle>,
    pub playlist_name: String,
    pub prepared_signal: u16,
}

#[derive(Clone)]
pub struct CreatePlaylistEntries {
    pub storage_id: StorageId,
    pub entries: Vec<StorageEntry>,
}

#[derive(Default, Clone, MistyState)]
pub struct CreatePlaylistState {
    pub picture: Option<MistyResourceHandle>,
    pub playlist_name: String,
    pub entries: Option<CreatePlaylistEntries>,
    pub mode: CreatePlaylistMode,
    pub recommend_playlist_names: Vec<String>,
    pub prepared_signal: u16,
    pub full_imported: bool,
}

#[derive(MistyAsyncTask)]
struct GeneralAsyncTask;

pub fn has_current_playlist(client: MistyClientHandle) -> bool {
    let current_playlist_id = CurrentPlaylistState::map(client, |state: &CurrentPlaylistState| {
        state.playlist.clone()
    });
    current_playlist_id.is_some()
}

pub fn get_current_playlist(client: MistyClientHandle) -> Option<Playlist> {
    let current_playlist_id = CurrentPlaylistState::map(client, |state: &CurrentPlaylistState| {
        state.playlist.clone()
    });
    current_playlist_id
}

pub fn reload_all_playlists_state(cx: MistyClientHandle) -> EaseResult<()> {
    let backend = get_backend(cx);
    GeneralAsyncTask::spawn(cx, |cx| async move {
        let metas = backend.send::<GetAllPlaylistAbstractsMsg>(()).await?;

        cx.schedule(|cx| {
            AllPlaylistState::update(cx, |state| {
                state.list = metas;
            });
            return EASE_RESULT_NIL;
        });

        return EASE_RESULT_NIL;
    });
    return EASE_RESULT_NIL;
}

pub fn reload_current_playlist_state(cx: MistyClientHandle) -> EaseResult<()> {
    let id = CurrentPlaylistState::map(cx, |state| state.playlist.map(|p| p.meta.id));
    let backend = get_backend(cx);
    GeneralAsyncTask::spawn(cx, |cx| async {
        let playlist = backend.send::<GetPlaylistMsg>(()).await?;

        cx.schedule(|cx| {
            CurrentPlaylistState::update(cx, |state| {
                state.playlist = playlist;
            });
            return EASE_RESULT_NIL;
        });

        return EASE_RESULT_NIL;
    });
    return EASE_RESULT_NIL;
}

pub fn change_current_playlist(app: MistyClientHandle, playlist_id: PlaylistId) {
    CurrentPlaylistState::update(app, |state| {
        state.playlist = Some(playlist_id);
    });
}

pub(super) fn prepare_import_entries_in_current_playlist(app: MistyClientHandle) -> EaseResult<()> {
    enter_storages_to_import(app, CurrentStorageImportType::Musics)?;
    Ok(())
}

pub fn remove_music_from_current_playlist(
    app: MistyClientHandle,
    music_id: MusicId,
) -> EaseResult<()> {
    let current_playlist_id = CurrentPlaylistState::map(app, |state| state.playlist.clone());
    if current_playlist_id.is_none() {
        return Ok(());
    }

    let playlist_id = current_playlist_id.unwrap();
    db_remove_music_from_playlist(app, playlist_id, music_id)?;

    update_playlist_state_by_remove_music(app, playlist_id, music_id)?;
    clear_current_music_state_if_invalid(app);
    Ok(())
}

pub fn remove_playlist(app: MistyClientHandle, id: PlaylistId) -> EaseResult<()> {
    db_remove_all_musics_in_playlist(app, id)?;
    db_remove_playlist(app, id)?;
    update_playlist_state_by_remove(app, id)?;
    clear_current_music_state_if_invalid(app);
    Ok(())
}

pub fn play_all_musics(app: MistyClientHandle) -> EaseResult<()> {
    let playlist = CurrentPlaylistState::map(app, |state| state.playlist.clone());
    let playlist_id = playlist.unwrap();
    if playlist.musics().is_empty() {
        return Ok(());
    }
    let musics = playlist.get_ordered_musics();
    let music = musics.first().unwrap();
    play_music(app, music.music_id())?;
    Ok(())
}

pub fn import_selected_entries_to_current_playlist(
    cx: MistyClientHandle,
    storage_id: StorageId,
    entries: Vec<StorageEntry>,
) -> EaseResult<()> {
    let backend = get_backend(cx);
    let playlist_id = get_playlist(cx)?;
    let entries = entries
        .into_iter()
        .filter(|entry| get_entry_type(&entry) == StorageEntryType::Music)
        .collect();
    let arg = ArgAddMusicsToPlaylist {
        id: playlist_id,
        entries: entries
            .into_iter()
            .map(|v| (StorageEntry {}, v.name))
            .collect(),
    };
    GeneralAsyncTask::spawn(cx, |cx| async {
        backend.send::<AddMusicsToPlaylistMsg>(arg).await?;

        cx.schedule(reload_current_playlist_state);
    });

    db_batch_add_music_to_playlist(
        cx,
        musics
            .iter()
            .map(|music| (music.id(), playlist_id))
            .collect(),
    )?;
    update_playlist_state_by_add_musics(cx, playlist_id, musics)?;
    Ok(())
}

// Edit Playlist

pub(super) fn finish_edit_playlist(app: MistyClientHandle) -> EaseResult<()> {
    let edit_state = EditPlaylistState::map(app, |state| state.clone());
    if edit_state.playlist_name.is_empty() {
        return Err(EaseError::OtherError(
            "playlist name cannot be empty".to_string(),
        ));
    }
    if edit_state.id.is_none() {
        return Err(EaseError::EditPlaylistNone);
    }

    let playlist_id = edit_state.id.unwrap();
    let picture = edit_state.picture.clone().map(|p| p.load().clone());

    let current_time_ms = TimerService::of(app).get_current_time_ms();
    db_upsert_playlist(
        app,
        ArgDBUpsertPlaylist {
            id: edit_state.id,
            title: edit_state.playlist_name.clone(),
            picture,
        },
        current_time_ms,
    )?;
    let playlist = get_playlist(app, playlist_id);
    if let Some(playlist) = playlist {
        AllPlaylistState::update(app, |state| {
            let mut playlist = Playlist::clone(&playlist);
            playlist.set_title(edit_state.playlist_name);
            playlist.set_self_picture(edit_state.picture);
            state.map.insert(playlist_id, Arc::new(playlist));
        });
    }
    Ok(())
}

pub fn clear_edit_playlist_state(app: MistyClientHandle) {
    ImportSelectedCoverInEditPlaylistAsyncTask::cancel_all(app);
    EditPlaylistState::update(app, |state| {
        state.picture = None;
        state.id = None;
    });
}

pub fn prepare_edit_playlist_state(app: MistyClientHandle, id: PlaylistId) -> EaseResult<()> {
    clear_create_playlist_state(app);

    let playlist = AllPlaylistState::map(app, |state| state.map.get(&id).map(|v| v.clone()))
        .ok_or(EaseError::EditPlaylistNone)?;

    EditPlaylistState::update(app, |state| {
        state.id = Some(id);
        state.playlist_name = playlist.title().to_string();
        state.picture = playlist.self_picture().clone();
        state.prepared_signal += 1;
    });
    Ok(())
}

pub fn update_edit_playlist_name(app: MistyClientHandle, name: String) {
    EditPlaylistState::update(app, |state| {
        state.playlist_name = name;
    });
}

pub fn clear_edit_playlist_cover(app: MistyClientHandle) {
    EditPlaylistState::update(app, |state| {
        state.picture = None;
    });
}

pub fn prepare_edit_playlist_cover(app: MistyClientHandle) -> EaseResult<()> {
    enter_storages_to_import(app, CurrentStorageImportType::EditPlaylistCover)?;
    return EASE_RESULT_NIL;
}

#[derive(Debug, MistyAsyncTask)]
struct ImportSelectedCoverInEditPlaylistAsyncTask;

pub fn import_selected_cover_in_edit_playlist(
    app: MistyClientHandle,
    storage_id: StorageId,
    entry: Entry,
) {
    ImportSelectedCoverInEditPlaylistAsyncTask::spawn_once(app, move |ctx| async move {
        let data = {
            let handle = ctx.handle();
            let handle = handle.handle();
            load_storage_entry_data(handle, storage_id, entry.path).await?
        };
        let buf = data.bytes().await?;

        ctx.schedule(|app| {
            let buf_handle = app.resource_manager().insert(buf);

            EditPlaylistState::update(app, |state| {
                state.picture = Some(buf_handle);
            });
            return EASE_RESULT_NIL;
        });
        return EASE_RESULT_NIL;
    });
}

// Create Playlist

pub(super) fn finish_create_playlist(app: MistyClientHandle) -> EaseResult<()> {
    let state = CreatePlaylistState::update(app, |state| state.clone());
    if state.playlist_name.is_empty() {
        return Err(EaseError::OtherError(
            "playlist name cannot be empty".to_string(),
        ));
    }

    clear_create_playlist_state(app);

    let picture = state.picture.map(|p| p.load().clone());

    let current_time_ms = TimerService::of(app).get_current_time_ms();
    let playlist_id = db_upsert_playlist(
        app,
        ArgDBUpsertPlaylist {
            id: None,
            title: state.playlist_name,
            picture,
        },
        current_time_ms,
    )?;

    if let Some(CreatePlaylistEntries {
        storage_id,
        entries,
    }) = state.entries
    {
        let musics = entries_to_musics(app, storage_id, entries)?;

        db_batch_add_music_to_playlist(
            app,
            musics
                .iter()
                .map(|music| (music.id(), playlist_id))
                .collect(),
        )?;

        schedule_download_musics_metadata_when_importing(app, musics.clone());
    }
    update_playlist_state_by_create(app, playlist_id)?;
    Ok(())
}

pub(crate) fn clear_create_playlist_state(app: MistyClientHandle) {
    ImportSelectedCoverInEditPlaylistAsyncTask::cancel_all(app);
    CreatePlaylistState::update(app, |state| {
        state.picture = None;
        state.entries = None;
        state.full_imported = false;
        state.mode = CreatePlaylistMode::Full;
        state.recommend_playlist_names.clear();
        state.playlist_name.clear();
    });
}

pub fn update_create_playlist_mode(app: MistyClientHandle, mode: CreatePlaylistMode) {
    CreatePlaylistState::update(app, |state| {
        state.mode = mode;
    });
}

pub fn update_create_playlist_name(app: MistyClientHandle, name: String) {
    CreatePlaylistState::update(app, |state| {
        state.playlist_name = name;
    });
}

pub(super) fn clear_create_playlist_cover(app: MistyClientHandle) {
    CreatePlaylistState::update(app, |state| {
        state.picture = None;
    });
}

pub(super) fn reset_create_playlist_full(app: MistyClientHandle) {
    clear_create_playlist_state(app);
}

pub fn prepare_import_cover_in_create_playlist(app: MistyClientHandle) -> EaseResult<()> {
    enter_storages_to_import(app, CurrentStorageImportType::CreatePlaylistCover)?;
    return EASE_RESULT_NIL;
}

pub fn prepare_import_entries_in_create_playlist(app: MistyClientHandle) -> EaseResult<()> {
    let playlist_mode = CreatePlaylistState::map(app, |state| state.mode);
    if playlist_mode != CreatePlaylistMode::Full {
        return Err(EaseError::OtherError(
            "can import entries in full mode only".to_string(),
        ));
    }

    enter_storages_to_import(app, CurrentStorageImportType::CreatePlaylistEntries)?;
    return EASE_RESULT_NIL;
}

pub fn get_playlist_musics_by_storage(
    client: MistyClientHandle,
    storage_id: StorageId,
) -> EaseResult<HashMap<PlaylistId, Vec<MusicId>>> {
    let map = db_get_playlist_music_tuples(client, storage_id)?;

    Ok(map)
}

pub fn remove_musics_in_playlists_by_storage(
    client: MistyClientHandle,
    storage_id: StorageId,
) -> EaseResult<()> {
    db_remove_musics_in_playlists_by_storage(client, storage_id)?;
    reload_all_playlist_state(client)?;
    Ok(())
}

#[derive(Debug, MistyAsyncTask)]
struct ImportSelectedCoverInCreatePlaylistAsyncTask;

pub fn import_selected_cover_in_create_playlist(
    app: MistyClientHandle,
    storage_id: StorageId,
    entry: Entry,
) {
    ImportSelectedCoverInCreatePlaylistAsyncTask::spawn_once(app, move |ctx| async move {
        let data = {
            let handle = ctx.handle();
            let handle = handle.handle();
            load_storage_entry_data(handle, storage_id, entry.path).await?
        };
        let buf = data.bytes().await?;

        ctx.schedule(|app| {
            let buf_handle = app.resource_manager().insert(buf);

            CreatePlaylistState::update(app, |state| {
                state.picture = Some(buf_handle);
            });
            return EASE_RESULT_NIL;
        });
        return EASE_RESULT_NIL;
    });
}

pub fn import_selected_entries_in_create_playlist(
    app: MistyClientHandle,
    storage_id: StorageId,
    entries: Vec<Entry>,
) -> EaseResult<()> {
    let mut music_entries: Vec<Entry> = Default::default();
    let mut cover_entry: Option<Entry> = None;

    for entry in entries.into_iter() {
        match get_entry_type(&entry) {
            StorageEntryType::Music => {
                music_entries.push(entry);
            }
            StorageEntryType::Image => {
                cover_entry = Some(entry);
            }
            _ => {}
        }
    }

    if let Some(entry) = cover_entry {
        import_selected_cover_in_create_playlist(app, storage_id, entry);
    }

    let mut recommend_playlist_names: HashSet<String> = Default::default();
    for entry in music_entries.iter() {
        let split: Vec<&str> = entry.path.split("/").collect();
        for i in 0..(split.len() - 1) {
            let p = split[i];
            if !p.is_empty() {
                recommend_playlist_names.insert(p.to_string());
            }
        }
    }

    let mut recommend_playlist_names: Vec<String> = recommend_playlist_names.into_iter().collect();
    recommend_playlist_names.sort_by(|a, b| b.len().cmp(&a.len()));

    CreatePlaylistState::update(app, |state| {
        state.entries = Some(CreatePlaylistEntries {
            storage_id,
            entries: music_entries,
        });
        state.recommend_playlist_names = recommend_playlist_names;
        if state.playlist_name.is_empty() && !state.recommend_playlist_names.is_empty() {
            state.playlist_name = state.recommend_playlist_names[0].clone();
        }
        state.full_imported = true;
        state.prepared_signal += 1;
    });
    Ok(())
}
