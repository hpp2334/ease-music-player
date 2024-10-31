use ease_client::{PlaylistListWidget, StorageListWidget, StorageUpsertWidget};
use ease_client_shared::backends::storage::StorageType;
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn storage_crud_1() {
    let app = TestApp::new("test-dbs/storage_crud_1", true).await;

    app.dispatch_click(StorageListWidget::Create);
    app.dispatch_click(StorageUpsertWidget::Type {
        value: StorageType::Webdav,
    });
    app.dispatch_change_text(StorageUpsertWidget::Address, "http://fake");
    app.dispatch_click(StorageUpsertWidget::IsAnonymous);
    app.dispatch_click(StorageUpsertWidget::Finish);
    app.wait_network().await;

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 1);
    let item = list.items[0].clone();
    assert_eq!(item.name, "http://fake");

    let id = item.storage_id;
    app.dispatch_click(StorageListWidget::Item { id });
    app.dispatch_change_text(StorageUpsertWidget::Address, "http://fake");
    app.dispatch_change_text(StorageUpsertWidget::Alias, "Demo");
    app.dispatch_click(StorageUpsertWidget::IsAnonymous);
    app.dispatch_click(StorageUpsertWidget::Finish);
    app.wait_network().await;

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 1);
    let item = list.items[0].clone();
    assert_eq!(item.storage_id, id);
    assert_eq!(item.name, "Demo");
}

#[tokio::test]
async fn storage_crud_2() {
    let app = TestApp::new("test-dbs/storage_crud_2", true).await;

    app.dispatch_click(StorageListWidget::Create);
    app.dispatch_click(StorageUpsertWidget::Type {
        value: StorageType::Webdav,
    });
    app.dispatch_change_text(StorageUpsertWidget::Address, "http://1");
    app.dispatch_click(StorageUpsertWidget::IsAnonymous);
    app.dispatch_click(StorageUpsertWidget::Finish);
    app.wait_network().await;

    app.dispatch_click(StorageListWidget::Create);
    app.dispatch_click(StorageUpsertWidget::Type {
        value: StorageType::Webdav,
    });
    app.dispatch_change_text(StorageUpsertWidget::Address, "http://2");
    app.dispatch_click(StorageUpsertWidget::IsAnonymous);
    app.dispatch_click(StorageUpsertWidget::Finish);
    app.wait_network().await;

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 2);
    assert_eq!(list.items[0].name, "http://1");
    assert_eq!(list.items[1].name, "http://2");

    let first_item_id = list.items[0].storage_id.clone();
    app.dispatch_click(StorageListWidget::Item { id: first_item_id });
    app.dispatch_click(StorageUpsertWidget::Remove);
    app.wait_network().await;

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].name, "http://2");
}

#[tokio::test]
async fn storage_remove_1() {
    let mut app = TestApp::new("test-dbs/storage_remove_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 1);
    let item = list.items[0].clone();
    assert_eq!(item.typ, StorageType::Webdav);

    let id = item.storage_id;
    app.dispatch_click(StorageListWidget::Item { id });

    let state = app.latest_state();
    let edit_storage = state.edit_storage.unwrap();
    assert_eq!(edit_storage.music_count, 2);
    assert_eq!(edit_storage.playlist_count, 1);
    assert_eq!(edit_storage.is_created, false);

    app.dispatch_click(StorageListWidget::Item { id });
    app.dispatch_click(StorageUpsertWidget::Remove);
    app.wait_network().await;

    let state = app.latest_state();
    let playlist_list = state.playlist_list.unwrap();
    app.dispatch_click(PlaylistListWidget::Item {
        id: playlist_list.playlist_list[0].id,
    });

    let state = app.latest_state();
    let current_playlist = state.current_playlist.unwrap();
    assert_eq!(current_playlist.items.len(), 0);
}
