use ease_client::view_models::*;
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn create_playlist_cover_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_cover_1", true);
    app.setup_preset(PresetDepth::Storage).await;

    app.call_controller(controller_prepare_edit_playlist_cover, ());
    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();
    let entry = state.entries[6].clone();
    assert_eq!(entry.name, "firefox.png");
    assert_eq!(entry.can_check, true);

    app.call_controller(controller_select_entry, entry.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;
    let state = app.latest_state().edit_playlist.unwrap();
    let picture = app.load_resource(state.picture.unwrap());
    assert_eq!(picture.len(), 82580);

    app.call_controller(controller_clear_edit_playlist_state, ());
    let state = app.latest_state().edit_playlist.unwrap();
    assert_eq!(state.picture, None);
}

#[tokio::test]
async fn edit_playlist_cover_1() {
    let mut app = TestApp::new("test-dbs/edit_playlist_cover_1", true);
    app.setup_preset(PresetDepth::Music).await;

    app.call_controller(controller_prepare_edit_playlist_cover, ());
    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();
    let entry = state.entries[6].clone();
    assert_eq!(entry.name, "firefox.png");
    assert_eq!(entry.can_check, true);

    app.call_controller(controller_select_entry, entry.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;
    let state = app.latest_state().edit_playlist.unwrap();
    let picture = app.load_resource(state.picture.unwrap());
    assert_eq!(picture.len(), 82580);

    app.call_controller(controller_clear_edit_playlist_state, ());
    let state = app.latest_state().edit_playlist.unwrap();
    assert_eq!(state.picture, None);
}

#[tokio::test]
async fn edit_playlist_cover_2() {
    let mut app = TestApp::new("test-dbs/edit_playlist_cover_2", true);
    app.setup_preset(PresetDepth::Music).await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    app.call_controller(controller_prepare_edit_playlist, state.playlist_list[0].id);
    app.call_controller(controller_prepare_edit_playlist_cover, ());
    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();
    let entry = state.entries[6].clone();
    assert_eq!(entry.name, "firefox.png");
    assert_eq!(entry.can_check, true);

    app.call_controller(controller_select_entry, entry.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network().await;
    app.call_controller(controller_finish_edit_playlist, ());
    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    let picture = app.load_resource(state.playlist_list[0].picture.unwrap());
    assert_eq!(picture.len(), 82580);

    // reload
    let app = TestApp::new("test-dbs/edit_playlist_cover_2", false);
    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    let picture = app.load_resource(state.playlist_list[0].picture.unwrap());
    assert_eq!(picture.len(), 82580);
}
