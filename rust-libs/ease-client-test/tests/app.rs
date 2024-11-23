use ease_client::MusicControlWidget;
use ease_client_shared::backends::player::PlayMode;
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn app_loaded_state_1() {
    {
        let mut app = TestApp::new("test-dbs/app_loaded_state_1", true).await;
        app.setup_preset(PresetDepth::Music).await;
    }

    let app = TestApp::new("test-dbs/app_loaded_state_1", false).await;
    let state = app.latest_state();

    let storage_list = state.storage_list.clone().unwrap();
    assert_eq!(storage_list.items.len(), 2);
    assert!(state.playlist_list.is_some());
    let playlist_list = state.playlist_list.clone().unwrap();
    assert_eq!(playlist_list.playlist_list.len(), 1);
}

#[tokio::test]
async fn app_loaded_state_2() {
    {
        let mut app = TestApp::new("test-dbs/app_loaded_state_2", true).await;
        app.setup_preset(PresetDepth::Music).await;
        app.dispatch_click(MusicControlWidget::Playmode);
        app.wait_network().await;
    }

    let app = TestApp::new("test-dbs/app_loaded_state_2", false).await;
    let state = app.latest_state();
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.play_mode, PlayMode::SingleLoop);
}
