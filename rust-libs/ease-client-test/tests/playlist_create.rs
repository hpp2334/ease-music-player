use ease_client::{PlaylistCreateWidget, PlaylistListWidget, StorageImportWidget};
use ease_client_shared::backends::playlist::CreatePlaylistMode;
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn create_playlist_full_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_full_1", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Tab {
        value: CreatePlaylistMode::Full,
    });
    app.dispatch_click(PlaylistCreateWidget::Import);

    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();

    let e1 = state.entries[4].clone();
    assert_eq!(e1.name, "angelical-pad-143276.mp3");
    assert_eq!(e1.can_check, true);
    let e2 = state.entries[6].clone();
    assert_eq!(e2.name, "firefox.png");
    assert_eq!(e2.can_check, true);

    app.dispatch_click(StorageImportWidget::StorageEntry { path: e1.path });
    app.dispatch_click(StorageImportWidget::StorageEntry { path: e2.path });
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;

    let state = app.latest_state().create_playlist.unwrap();
    assert_eq!(state.mode, CreatePlaylistMode::Full);
    assert_eq!(state.music_count, 1);
    assert_eq!(state.recommend_playlist_names.len(), 0);
    let picture = app.load_resource(&state.picture).await;
    assert_eq!(picture.len(), 82580);

    app.dispatch_change_text(PlaylistCreateWidget::Name, "ABC");
    app.dispatch_click(PlaylistCreateWidget::FinishCreate);
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC".to_string());
    assert_ne!(state.playlist_list[0].cover_url, "")
}

#[tokio::test]
async fn create_playlist_full_2() {
    let mut app = TestApp::new("test-dbs/create_playlist_full_2", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Tab {
        value: CreatePlaylistMode::Full,
    });
    app.dispatch_click(PlaylistCreateWidget::Import);
    app.wait_network().await;
    app.dispatch_click(StorageImportWidget::StorageEntry {
        path: app.latest_state().current_storage_entries.unwrap().entries[0]
            .path
            .clone(),
    });
    app.wait_network().await;

    let state = app.latest_state().current_storage_entries.unwrap();

    let e1 = state.entries[0].clone();
    assert_eq!(e1.name, "angelical-pad-143276.mp3");
    assert_eq!(e1.can_check, true);

    app.dispatch_click(StorageImportWidget::StorageEntry { path: e1.path });
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;

    let state = app.latest_state().create_playlist.unwrap();
    assert_eq!(state.mode, CreatePlaylistMode::Full);
    assert_eq!(state.music_count, 1);
    assert_eq!(state.recommend_playlist_names, vec!["musics".to_string()]);
    assert_eq!(state.picture, "");

    app.dispatch_change_text(PlaylistCreateWidget::Name, "ABC");
    app.dispatch_click(PlaylistCreateWidget::FinishCreate);
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC".to_string());
    assert_eq!(state.playlist_list[0].cover_url, "".to_string())
}

#[tokio::test]
async fn create_playlist_empty_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_empty_1", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Tab {
        value: CreatePlaylistMode::Empty,
    });

    app.dispatch_change_text(PlaylistCreateWidget::Name, "ABC");
    app.dispatch_click(PlaylistCreateWidget::FinishCreate);
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC".to_string());
    assert_eq!(state.playlist_list[0].cover_url, "")
}

#[tokio::test]
async fn create_playlist_only_cover_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_only_cover_1", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Tab {
        value: CreatePlaylistMode::Full,
    });
    app.dispatch_click(PlaylistCreateWidget::Import);

    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();

    let e2 = state.entries[6].clone();
    assert_eq!(e2.name, "firefox.png");

    app.dispatch_click(StorageImportWidget::StorageEntry { path: e2.path });
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;

    let state = app.latest_state().create_playlist.unwrap();
    assert_eq!(state.mode, CreatePlaylistMode::Full);
    assert_eq!(state.music_count, 0);
    assert_eq!(state.recommend_playlist_names.len(), 0);
    let picture = app.load_resource(&state.picture).await;
    assert_eq!(picture.len(), 82580);
}
