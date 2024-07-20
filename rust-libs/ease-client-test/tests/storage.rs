use ease_client::modules::*;
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn storage_crud_1() {
    let app = TestApp::new("test-dbs/storage_crud_1", true);

    app.call_controller(
        controller_upsert_storage,
        ArgUpsertStorage {
            id: None,
            addr: "http://fake".to_string(),
            alias: None,
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            typ: StorageType::Webdav,
        },
    );

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 2);
    let item = list.items[0].clone();
    assert_eq!(item.name, "http://fake");

    let id = item.storage_id;
    app.call_controller(controller_prepare_edit_storage, Some(id));
    app.call_controller(
        controller_upsert_storage,
        ArgUpsertStorage {
            id: Some(id.clone()),
            addr: "http://fake".to_string(),
            alias: Some("Demo".to_string()),
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            typ: StorageType::Webdav,
        },
    );
    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 2);
    let item = list.items[0].clone();
    assert_eq!(item.storage_id, id);
    assert_eq!(item.name, "Demo");
}

#[tokio::test]
async fn storage_crud_2() {
    let app = TestApp::new("test-dbs/storage_crud_2", true);

    app.call_controller(
        controller_upsert_storage,
        ArgUpsertStorage {
            id: None,
            addr: "http://1".to_string(),
            alias: None,
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            typ: StorageType::Webdav,
        },
    );
    app.call_controller(
        controller_upsert_storage,
        ArgUpsertStorage {
            id: None,
            addr: "http://2".to_string(),
            alias: None,
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            typ: StorageType::Webdav,
        },
    );

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 3);
    assert_eq!(list.items[0].name, "http://1");
    assert_eq!(list.items[1].name, "http://2");

    let first_item_id = list.items[0].storage_id.clone();
    app.call_controller(controller_remove_storage, first_item_id);

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 2);
    assert_eq!(list.items[0].name, "http://2");
}

#[tokio::test]
async fn storage_remove_1() {
    let mut app = TestApp::new("test-dbs/storage_remove_1", true);
    app.setup_preset(PresetDepth::Music).await;

    let state = app.latest_state();
    let list = state.storage_list.clone().unwrap_or_default();
    assert_eq!(list.items.len(), 2);
    let item = list.items[0].clone();
    assert_eq!(item.typ, StorageType::Webdav);

    let id = item.storage_id;
    app.call_controller(controller_prepare_edit_storage, Some(id));

    let state = app.latest_state().edit_storage.unwrap();
    assert_eq!(state.music_count, 2);
    assert_eq!(state.playlist_count, 1);
    assert_eq!(state.is_created, false);

    app.call_controller(controller_remove_storage, id);

    let state = app.latest_state().playlist_list.unwrap();
    app.call_controller(
        controller_change_current_playlist,
        state.playlist_list[0].id,
    );

    let state = app.latest_state().current_playlist.unwrap();
    assert_eq!(state.items.len(), 0);
}
