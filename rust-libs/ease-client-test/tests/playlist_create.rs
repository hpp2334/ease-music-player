use ease_client::view_models::*;
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn create_playlist_full_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_full_1", true);
    app.setup_preset(PresetDepth::Storage).await;

    app.call_controller(controller_prepare_create_playlist, ());
    app.call_controller(
        controller_update_create_playlist_mode,
        CreatePlaylistMode::Full,
    );
    app.call_controller(controller_prepare_create_playlist_entries, ());

    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();

    let e1 = state.entries[4].clone();
    assert_eq!(e1.name, "angelical-pad-143276.mp3");
    assert_eq!(e1.can_check, true);
    let e2 = state.entries[6].clone();
    assert_eq!(e2.name, "firefox.png");
    assert_eq!(e2.can_check, true);

    app.call_controller(controller_select_entry, e1.path);
    app.call_controller(controller_select_entry, e2.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;

    let state = app.latest_state().create_playlist.unwrap();
    assert_eq!(state.mode, CreatePlaylistMode::Full);
    assert_eq!(state.music_count, 1);
    assert_eq!(state.recommend_playlist_names.len(), 0);
    let picture = app.load_resource(state.picture.unwrap());
    assert_eq!(picture.len(), 82580);

    app.call_controller(controller_update_create_playlist_name, "ABC".to_string());
    app.call_controller(controller_finish_create_playlist, ());
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC".to_string());
    assert_ne!(state.playlist_list[0].picture, None)
}

#[tokio::test]
async fn create_playlist_full_2() {
    let mut app = TestApp::new("test-dbs/create_playlist_full_2", true);
    app.setup_preset(PresetDepth::Storage).await;

    app.call_controller(controller_prepare_create_playlist, ());
    app.call_controller(
        controller_update_create_playlist_mode,
        CreatePlaylistMode::Full,
    );
    app.call_controller(controller_prepare_create_playlist_entries, ());
    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();
    app.call_controller(controller_locate_entry, state.entries[0].path.clone());
    app.wait_network().await;

    let state = app.latest_state().current_storage_entries.unwrap();
    let e1 = state.entries[0].clone();
    assert_eq!(e1.name, "angelical-pad-143276.mp3");
    assert_eq!(e1.can_check, true);

    app.call_controller(controller_select_entry, e1.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;

    let state = app.latest_state().create_playlist.unwrap();
    assert_eq!(state.mode, CreatePlaylistMode::Full);
    assert_eq!(state.music_count, 1);
    assert_eq!(state.recommend_playlist_names, vec!["musics".to_string()]);
    assert_eq!(state.picture, None);

    app.call_controller(controller_update_create_playlist_name, "ABC".to_string());
    app.call_controller(controller_finish_create_playlist, ());
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC".to_string());
    assert_eq!(state.playlist_list[0].picture, None)
}

#[tokio::test]
async fn create_playlist_empty_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_empty_1", true);
    app.setup_preset(PresetDepth::Storage).await;

    app.call_controller(controller_prepare_create_playlist, ());
    app.call_controller(
        controller_update_create_playlist_mode,
        CreatePlaylistMode::Empty,
    );

    app.call_controller(controller_update_create_playlist_name, "ABC".to_string());
    app.call_controller(controller_finish_create_playlist, ());
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC".to_string());
    assert_eq!(state.playlist_list[0].picture, None)
}

#[tokio::test]
async fn create_playlist_only_cover_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_only_cover_1", true);
    app.setup_preset(PresetDepth::Storage).await;

    app.call_controller(controller_prepare_create_playlist, ());
    app.call_controller(
        controller_update_create_playlist_mode,
        CreatePlaylistMode::Full,
    );
    app.call_controller(controller_prepare_create_playlist_entries, ());

    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();

    let e2 = state.entries[6].clone();
    assert_eq!(e2.name, "firefox.png");

    app.call_controller(controller_select_entry, e2.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;

    let state = app.latest_state().create_playlist.unwrap();
    assert_eq!(state.mode, CreatePlaylistMode::Full);
    assert_eq!(state.music_count, 0);
    assert_eq!(state.recommend_playlist_names.len(), 0);
    let picture = app.load_resource(state.picture.unwrap());
    assert_eq!(picture.len(), 82580);
}
