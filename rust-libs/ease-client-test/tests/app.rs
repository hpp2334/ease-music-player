use ease_client::view_models::{controller_update_music_playmode_to_next, PlayMode};
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn app_loaded_state_1() {
    let mut app = TestApp::new("test-dbs/app_loaded_state_1", true);
    app.setup_preset(PresetDepth::Music).await;

    let app = TestApp::new("test-dbs/app_loaded_state_1", false);
    let state = app.latest_state();

    let storage_list = state.storage_list.clone().unwrap();
    assert_eq!(storage_list.items.len(), 2);
    assert!(state.playlist_list.is_some());
    let playlist_list = state.playlist_list.clone().unwrap();
    assert_eq!(playlist_list.playlist_list.len(), 1);
}

#[tokio::test]
async fn app_loaded_state_2() {
    let mut app = TestApp::new("test-dbs/app_loaded_state_2", true);
    app.setup_preset(PresetDepth::Music).await;
    app.call_controller(controller_update_music_playmode_to_next, ());

    let app = TestApp::new("test-dbs/app_loaded_state_2", false);
    let state = app.latest_state();
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.play_mode, PlayMode::SingleLoop);
}
