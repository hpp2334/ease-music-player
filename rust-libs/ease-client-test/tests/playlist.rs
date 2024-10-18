use ease_client::view_models::*;
use ease_client_test::{PresetDepth, ReqInteceptor, TestApp};

fn create_playlist(app: &TestApp, name: &str) {
    app.call_controller(controller_prepare_create_playlist, ());
    app.call_controller(
        controller_update_create_playlist_mode,
        CreatePlaylistMode::Empty,
    );
    app.call_controller(controller_update_create_playlist_name, name.to_string());
    app.call_controller(controller_finish_create_playlist, ());
}

#[tokio::test]
async fn playlist_crud_1() {
    let app = TestApp::new("test-dbs/playlist_crud_1", true);

    create_playlist(&app, "Playlist A");
    app.advance_timer(1).await;
    create_playlist(&app, "Playlist B");
    let state = app.latest_state();
    let list = state.playlist_list.clone().unwrap_or_default();
    assert_eq!(list.playlist_list.len(), 2);
    let item = list.playlist_list[0].clone();
    assert_eq!(item.title, "Playlist B");
    let item = list.playlist_list[1].clone();
    let id = item.id.clone();
    assert_eq!(item.title, "Playlist A");

    app.advance_timer(1).await;

    app.call_controller(controller_prepare_edit_playlist, id);
    app.call_controller(
        controller_update_edit_playlist_name,
        "Playlist C".to_string(),
    );
    app.call_controller(controller_finish_edit_playlist, ());
    let state = app.latest_state();
    let list = state.playlist_list.clone().unwrap_or_default();
    assert_eq!(list.playlist_list.len(), 2);
    let item = list.playlist_list[0].clone();
    assert_eq!(item.title, "Playlist B");
    let item = list.playlist_list[1].clone();
    assert_eq!(item.title, "Playlist C");

    app.call_controller(controller_remove_playlist, item.id.clone());
    let state = app.latest_state();
    let list = state.playlist_list.clone().unwrap_or_default();
    assert_eq!(list.playlist_list.len(), 1);
    let item = list.playlist_list[0].clone();
    assert_eq!(item.title, "Playlist B");
}

#[tokio::test]
async fn playlist_import_select_non_music_1() {
    let mut app = TestApp::new("test-dbs/playlist_import_select_non_music_1", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
    app.call_controller(controller_change_current_playlist, playlist_id);

    let storage_id = app.get_first_storage_id_from_latest_state();
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.call_controller(controller_select_storage_in_import, storage_id);
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.entries.len(), 7);
    let item = &entries.entries[2];
    assert_eq!(item.path, "/README.md");
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);

    app.call_controller(controller_select_entry, entries.entries[2].path.clone());
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    let item = &entries.entries[2];
    assert_eq!(item.path, "/README.md");
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);
    assert_eq!(entries.selected_count, 0);
}

#[tokio::test]
async fn playlist_import_musics_1() {
    let mut app = TestApp::new("test-dbs/playlist_import_musics_1", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
    app.call_controller(controller_change_current_playlist, playlist_id);

    let storage_id = app.get_first_storage_id_from_latest_state();
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.call_controller(controller_select_storage_in_import, storage_id);
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.entries.len(), 7);
    let item = &entries.entries[0];
    assert_eq!(item.name, "musics");
    assert_eq!(item.path, "/musics");
    assert_eq!(item.is_folder, true);
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);
    let item = &entries.entries[1];
    assert_eq!(item.name, "sounds");
    assert_eq!(item.path, "/sounds");
    assert_eq!(item.is_folder, true);
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);
    let item = &entries.entries[2];
    assert_eq!(item.name, "README.md");
    assert_eq!(item.path, "/README.md");
    assert_eq!(item.is_folder, false);
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);
    let item = &entries.entries[4];
    assert_eq!(item.name, "angelical-pad-143276.mp3");
    assert_eq!(item.path, "/angelical-pad-143276.mp3");
    assert_eq!(item.is_folder, false);
    assert_eq!(item.can_check, true);
    assert_eq!(item.checked, false);

    app.call_controller(controller_select_entry, entries.entries[4].path.clone());
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    let item = &entries.entries[4];
    assert_eq!(item.name, "angelical-pad-143276.mp3");
    assert_eq!(item.path, "/angelical-pad-143276.mp3");
    assert_eq!(item.is_folder, false);
    assert_eq!(item.can_check, true);
    assert_eq!(item.checked, true);
    assert_eq!(entries.selected_count, 1);

    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_playlist.clone().unwrap();
    assert_eq!(state.duration, "00:00:24");
    assert_eq!(state.items.len(), 1);
    let item = state.items[0].clone();
    assert_eq!(item.title, "angelical-pad-143276");
}

#[tokio::test]
async fn playlist_import_musics_2() {
    let mut app = TestApp::new("test-dbs/playlist_import_musics_2", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
    app.call_controller(controller_change_current_playlist, playlist_id);

    let storage_id = app.get_first_storage_id_from_latest_state();
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.call_controller(controller_select_storage_in_import, storage_id);
    app.wait_network().await;

    app.call_controller(controller_toggle_all_checked_entries, ());
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.selected_count, 2);

    app.call_controller(controller_toggle_all_checked_entries, ());
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.selected_count, 0);
}

#[tokio::test]
async fn playlist_import_musics_3() {
    let mut app = TestApp::new("test-dbs/playlist_import_musics_3", true);
    app.setup_preset(PresetDepth::Music).await;
    app.call_controller(
        controller_change_current_playlist,
        app.get_first_playlist_id_from_latest_state(),
    );

    let state = app.latest_state();
    let state = state.current_playlist.unwrap();
    assert_eq!(state.items.len(), 2);

    app.call_controller(
        controller_remove_music_from_current_playlist,
        app.get_first_music_id_from_latest_state(),
    );
    let state = app.latest_state();
    let state = state.current_playlist.unwrap();
    assert_eq!(state.items.len(), 1);
}

#[tokio::test]
async fn playlist_import_from_local_1() {
    let mut app = TestApp::new("test-dbs/playlist_import_from_local_1", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.call_controller(controller_change_current_playlist, playlist_id);
    app.call_controller(controller_update_storage_permission, true);

    let state = app.latest_state();
    let state = state.storage_list.unwrap();
    assert_eq!(state.items[0].typ, StorageType::Webdav);
    assert_eq!(state.items[1].typ, StorageType::Local);

    let storage_id = state.items[1].clone().storage_id.clone();
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.call_controller(controller_select_storage_in_import, storage_id);
    app.wait_network().await;

    let cwd = std::env::current_dir().unwrap().join("test-files");
    let cwd = cwd.to_string_lossy().to_string().replace('\\', "/");
    app.call_controller(controller_locate_entry, cwd);
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.entries.len(), 7);
    let item = &entries.entries[0];
    assert_eq!(item.name, "musics");
    assert_eq!(item.is_folder, true);
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);

    app.call_controller(controller_select_entry, entries.entries[4].path.clone());
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_playlist.clone().unwrap();
    assert_eq!(state.duration, "00:00:24");
    assert_eq!(state.items.len(), 1);
    let item = state.items[0].clone();
    assert_eq!(item.title, "angelical-pad-143276");

    app.call_controller(controller_play_all_musics, ());
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.total_duration, "00:00:24");
}

#[tokio::test]
async fn playlist_import_need_permission() {
    let mut app = TestApp::new("test-dbs/playlist_import_need_permission", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.call_controller(controller_change_current_playlist, playlist_id);

    // local storage id
    let state = app.latest_state();
    let state = state.storage_list.unwrap();
    let storage_id = state.items[1].clone().storage_id.clone();
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.call_controller(controller_select_storage_in_import, storage_id);
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(state.state_type, CurrentStorageStateType::NeedPermission);

    app.call_controller(controller_update_storage_permission, true);
    app.call_controller(controller_refresh_current_storage_in_import, ());
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(state.state_type, CurrentStorageStateType::OK);
}

#[tokio::test]
async fn playlist_import_authentication() {
    let mut app = TestApp::new("test-dbs/playlist_import_authentication", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.call_controller(controller_change_current_playlist, playlist_id);
    app.wait_network().await;
    app.set_inteceptor_req(Some(ReqInteceptor::AuthenticationFailed));
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(
        state.state_type,
        CurrentStorageStateType::AuthenticationFailed
    );
}

#[tokio::test]
async fn playlist_import_other_error() {
    let mut app = TestApp::new("test-dbs/playlist_import_other_error", true);
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.call_controller(controller_change_current_playlist, playlist_id);
    app.wait_network().await;
    app.set_inteceptor_req(Some(ReqInteceptor::InternalError));
    app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(state.state_type, CurrentStorageStateType::UnknownError);
}

#[tokio::test]
async fn playlist_full_reimport_discarded_bug() {
    let mut app = TestApp::new("test-dbs/playlist_full_reimport_discarded_bug", true);
    app.setup_preset(PresetDepth::Storage).await;

    let create_playlist_and_import_music = || async {
        create_playlist(&app, "A");

        let state = app.latest_state();
        let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
        app.call_controller(controller_change_current_playlist, playlist_id);

        let storage_id = app.get_first_storage_id_from_latest_state();
        app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
        app.call_controller(controller_select_storage_in_import, storage_id);
        app.wait_network().await;
        let entries = app.latest_state().current_storage_entries.unwrap();
        app.call_controller(controller_select_entry, entries.entries[4].path.clone());
        let state = app.latest_state();
        let entries = state.current_storage_entries.unwrap();
        let item = &entries.entries[4];
        assert_eq!(item.name, "angelical-pad-143276.mp3");
        assert_eq!(item.path, "/angelical-pad-143276.mp3");
        assert_eq!(item.is_folder, false);
        assert_eq!(item.can_check, true);
        assert_eq!(item.checked, true);
        assert_eq!(entries.selected_count, 1);

        app.call_controller(controller_finish_selected_entries_in_import, ());
        app.wait_network().await;
        let state = app.latest_state();
        let state = state.current_playlist.clone().unwrap();
        assert_eq!(state.duration, "00:00:24");
        assert_eq!(state.items.len(), 1);
        let item = state.items[0].clone();
        assert_eq!(item.title, "angelical-pad-143276");

        return playlist_id;
    };

    let playlist_id = create_playlist_and_import_music().await;
    app.call_controller(controller_remove_playlist, playlist_id);
    create_playlist_and_import_music().await;

    // reload
    let app = TestApp::new("test-dbs/playlist_full_reimport_discarded_bug", false);
    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].count, 1);
}
