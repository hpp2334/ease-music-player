use ease_client::{
    PlaylistCreateWidget, PlaylistDetailWidget, PlaylistEditWidget, PlaylistListWidget,
    StorageImportWidget, StorageListWidget, StorageUpsertWidget,
};
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn create_playlist_cover_1() {
    let mut app = TestApp::new("test-dbs/create_playlist_cover_1", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Cover);
    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();
    let entry = state.entries[6].clone();
    assert_eq!(entry.name, "firefox.png");
    assert_eq!(entry.can_check, true);

    app.dispatch_click(StorageImportWidget::StorageEntry { path: entry.path });
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;
    let state = app.latest_state().create_playlist.unwrap();
    let picture = app.load_resource_by_key(state.picture.unwrap()).await;
    assert_eq!(picture.len(), 82580);
    app.dispatch_click(PlaylistCreateWidget::FinishCreate);
    app.wait_network().await;

    app.dispatch_click(PlaylistListWidget::Item {
        id: app.latest_state().playlist_list.unwrap().playlist_list[0].id,
    });
    app.wait_network().await;
    app.dispatch_click(PlaylistDetailWidget::Edit);
    app.dispatch_click(PlaylistEditWidget::ClearCover);
    app.dispatch_click(PlaylistEditWidget::FinishEdit);
    let state = app.latest_state().edit_playlist.unwrap();
    assert_eq!(state.picture, None);
}

#[tokio::test]
async fn edit_playlist_cover_1() {
    let mut app = TestApp::new("test-dbs/edit_playlist_cover_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    app.dispatch_click(PlaylistDetailWidget::Edit);
    app.dispatch_click(PlaylistEditWidget::Cover);
    app.wait_network().await;
    let state = app.latest_state().current_storage_entries.unwrap();
    let entry = state.entries[6].clone();
    assert_eq!(entry.name, "firefox.png");
    assert_eq!(entry.can_check, true);

    app.dispatch_click(StorageImportWidget::StorageEntry { path: entry.path });
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;
    let state = app.latest_state().edit_playlist.unwrap();
    let picture = app.load_resource_by_key(state.picture.unwrap()).await;
    assert_eq!(picture.len(), 82580);

    app.dispatch_click(PlaylistEditWidget::ClearCover);
    app.dispatch_click(PlaylistEditWidget::FinishEdit);
    let state = app.latest_state().edit_playlist.unwrap();
    assert_eq!(state.picture, None);
}

#[tokio::test]
async fn edit_playlist_cover_2() {
    {
        let mut app = TestApp::new("test-dbs/edit_playlist_cover_2", true).await;
        app.setup_preset(PresetDepth::Music).await;

        let state = app.latest_state().playlist_list.unwrap();
        assert_eq!(state.playlist_list.len(), 1);
        app.dispatch_click(PlaylistListWidget::Item {
            id: state.playlist_list[0].id,
        });
        app.wait_network().await;
        app.dispatch_click(PlaylistDetailWidget::Edit);
        app.dispatch_click(PlaylistEditWidget::Cover);
        app.wait_network().await;
        let state = app.latest_state().current_storage_entries.unwrap();
        let entry = state.entries[6].clone();
        assert_eq!(entry.name, "firefox.png");
        assert_eq!(entry.can_check, true);

        app.dispatch_click(StorageImportWidget::StorageEntry { path: entry.path });
        app.dispatch_click(StorageImportWidget::Import);
        app.wait_network().await;
        app.dispatch_click(PlaylistEditWidget::FinishEdit);
        app.wait_network().await;

        let state = app.latest_state().playlist_list.unwrap();
        assert_eq!(state.playlist_list.len(), 1);
        let picture = app
            .load_resource_by_key(state.playlist_list[0].cover.clone().unwrap())
            .await;
        assert_eq!(picture.len(), 82580);
    }

    // reload
    let app = TestApp::new("test-dbs/edit_playlist_cover_2", false).await;
    app.dispatch_click(StorageListWidget::Item {
        id: app.get_first_storage_id_from_latest_state(),
    });
    app.dispatch_change_text(StorageUpsertWidget::Address, app.fake_server_addr());
    app.dispatch_click(StorageUpsertWidget::Finish);
    app.wait_network().await;

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    let picture = app
        .load_resource_by_key(state.playlist_list[0].cover.clone().unwrap())
        .await;
    assert_eq!(picture.len(), 82580);
}
