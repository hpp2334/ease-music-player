use ease_client::{
    view_models::storage::import::StorageImportAction, Action, PlaylistCreateWidget,
    PlaylistDetailWidget, PlaylistEditWidget, PlaylistListWidget, StorageImportWidget,
    StorageListWidget, ViewAction,
};
use ease_client_shared::backends::{
    playlist::CreatePlaylistMode,
    storage::{CurrentStorageStateType, StorageType},
};
use ease_client_test::{PresetDepth, ReqInteceptor, TestApp};

async fn create_playlist(app: &TestApp, name: &str) {
    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Tab {
        value: CreatePlaylistMode::Empty,
    });
    app.dispatch_change_text(PlaylistCreateWidget::Name, name);
    app.dispatch_click(PlaylistCreateWidget::FinishCreate);
    app.wait_network().await;
}

#[tokio::test]
async fn playlist_crud_1() {
    let mut app = TestApp::new("test-dbs/playlist_crud_1", true).await;

    create_playlist(&mut app, "Playlist A").await;
    app.advance_timer(1).await;
    create_playlist(&mut app, "Playlist B").await;
    let state = app.latest_state();
    let list = state.playlist_list.clone().unwrap_or_default();
    assert_eq!(list.playlist_list.len(), 2);
    let item = list.playlist_list[0].clone();
    assert_eq!(item.title, "Playlist B");
    let item = list.playlist_list[1].clone();
    let id = item.id.clone();
    assert_eq!(item.title, "Playlist A");

    app.advance_timer(1).await;

    app.dispatch_click(PlaylistListWidget::Item { id });
    app.wait_network().await;
    app.dispatch_click(PlaylistDetailWidget::Edit);
    app.dispatch_change_text(PlaylistEditWidget::Name, "Playlist C");
    app.dispatch_click(PlaylistEditWidget::FinishEdit);
    app.wait_network().await;
    let state = app.latest_state();
    let list = state.playlist_list.clone().unwrap_or_default();
    assert_eq!(list.playlist_list.len(), 2);
    let item = list.playlist_list[0].clone();
    assert_eq!(item.title, "Playlist B");
    let item = list.playlist_list[1].clone();
    assert_eq!(item.title, "Playlist C");

    app.dispatch_click(PlaylistListWidget::Item { id: item.id });
    app.wait_network().await;
    app.dispatch_click(PlaylistDetailWidget::Remove);
    app.wait_network().await;
    let state = app.latest_state();
    let list = state.playlist_list.clone().unwrap_or_default();
    assert_eq!(list.playlist_list.len(), 1);
    let item = list.playlist_list[0].clone();
    assert_eq!(item.title, "Playlist B");
}

#[tokio::test]
async fn playlist_import_select_non_music_1() {
    let mut app = TestApp::new("test-dbs/playlist_import_select_non_music_1", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;

    let storage_id = app.get_first_storage_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Import);
    app.wait_network().await;
    app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert!(!entries.can_undo);
    assert_eq!(entries.entries.len(), 7);
    let item = &entries.entries[2];
    assert_eq!(item.path, "/README.md");
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);

    app.dispatch_click(StorageImportWidget::StorageEntry {
        path: entries.entries[2].path.clone(),
    });
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert!(!entries.can_undo);
    let item = &entries.entries[2];
    assert_eq!(item.path, "/README.md");
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);
    assert_eq!(entries.selected_count, 0);

    app.emit(Action::View(ViewAction::StorageImport(
        StorageImportAction::Undo,
    )));
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert!(!entries.can_undo);
    assert_eq!(entries.entries.len(), 7);
}

#[tokio::test]
async fn playlist_import_musics_1() {
    let mut app = TestApp::new("test-dbs/playlist_import_musics_1", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;

    let storage_id = app.get_first_storage_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Import);
    app.wait_network().await;
    app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
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
    app.dispatch_click(StorageImportWidget::StorageEntry {
        path: entries.entries[4].path.clone(),
    });
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    let item = &entries.entries[4];
    assert_eq!(item.name, "angelical-pad-143276.mp3");
    assert_eq!(item.path, "/angelical-pad-143276.mp3");
    assert_eq!(item.is_folder, false);
    assert_eq!(item.can_check, true);
    assert_eq!(item.checked, true);
    assert_eq!(entries.selected_count, 1);

    app.dispatch_click(StorageImportWidget::Import);
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
    let mut app = TestApp::new("test-dbs/playlist_import_musics_2", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;

    let storage_id = app.get_first_storage_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Import);
    app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
    app.wait_network().await;

    app.dispatch_click(StorageImportWidget::ToggleAll);
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.selected_count, 2);

    app.dispatch_click(StorageImportWidget::ToggleAll);
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.selected_count, 0);
}
#[tokio::test]
async fn playlist_import_musics_3() {
    let mut app = TestApp::new("test-dbs/playlist_import_musics_3", true).await;
    app.setup_preset(PresetDepth::Music).await;
    app.dispatch_click(PlaylistListWidget::Item {
        id: app.get_first_playlist_id_from_latest_state(),
    });
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_playlist.unwrap();
    assert_eq!(state.items.len(), 2);

    app.dispatch_click(PlaylistDetailWidget::RemoveMusic {
        id: app.get_first_music_id_from_latest_state(),
    });
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_playlist.unwrap();
    assert_eq!(state.items.len(), 1);
}

#[tokio::test]
async fn playlist_import_from_local_1() {
    let mut app = TestApp::new("test-dbs/playlist_import_from_local_1", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.permission().update_permission(true);

    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;

    app.dispatch_click(PlaylistDetailWidget::Edit);
    app.dispatch_click(PlaylistDetailWidget::Import);
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.storage_list.unwrap();
    assert_eq!(state.items[0].typ, StorageType::Webdav);
    assert_eq!(state.items[1].typ, StorageType::Local);

    let storage_id = state.items[1].clone().storage_id.clone();
    app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
    app.wait_network().await;

    let cwd = std::env::current_dir().unwrap().join("test-files");
    let cwd = cwd.to_string_lossy().to_string().replace('\\', "/");
    app.dispatch_click(StorageImportWidget::StorageEntry { path: cwd });
    app.wait_network().await;
    let state = app.latest_state();
    let entries = state.current_storage_entries.unwrap();
    assert_eq!(entries.entries.len(), 7);
    let item = &entries.entries[0];
    assert_eq!(item.name, "musics");
    assert_eq!(item.is_folder, true);
    assert_eq!(item.can_check, false);
    assert_eq!(item.checked, false);

    app.dispatch_click(StorageImportWidget::StorageEntry {
        path: entries.entries[4].path.clone(),
    });
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_playlist.clone().unwrap();
    assert_eq!(state.duration, "00:00:24");
    assert_eq!(state.items.len(), 1);
    let item = state.items[0].clone();
    assert_eq!(item.title, "angelical-pad-143276");

    app.dispatch_click(PlaylistDetailWidget::PlayAll);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.total_duration, "00:00:24");
}

#[tokio::test]
async fn playlist_import_need_permission() {
    let mut app = TestApp::new("test-dbs/playlist_import_need_permission", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;

    // local storage id
    let state = app.latest_state();
    let state = state.storage_list.unwrap();
    let storage_id = state.items[1].clone().storage_id.clone();
    app.dispatch_click(PlaylistDetailWidget::Import);
    app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(state.state_type, CurrentStorageStateType::NeedPermission);

    app.permission().update_permission(true);
    app.dispatch_click(StorageImportWidget::Error);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(state.state_type, CurrentStorageStateType::OK);
}

#[tokio::test]
async fn playlist_import_authentication() {
    let mut app = TestApp::new("test-dbs/playlist_import_authentication", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;
    app.set_inteceptor_req(Some(ReqInteceptor::AuthenticationFailed));
    app.dispatch_click(PlaylistDetailWidget::Import);
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
    let mut app = TestApp::new("test-dbs/playlist_import_other_error", true).await;
    app.setup_preset(PresetDepth::Playlist).await;

    let state = app.latest_state();
    let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();

    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;
    app.set_inteceptor_req(Some(ReqInteceptor::InternalError));
    app.dispatch_click(PlaylistDetailWidget::Import);
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_storage_entries.unwrap();
    assert_eq!(state.state_type, CurrentStorageStateType::UnknownError);
}

#[tokio::test]
async fn playlist_full_reimport_discarded_bug() {
    let mut app = TestApp::new("test-dbs/playlist_full_reimport_discarded_bug", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    let create_playlist_and_import_music = || async {
        create_playlist(&app, "A").await;

        let state = app.latest_state();
        let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
        app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
        app.wait_network().await;

        let storage_id = app.get_first_storage_id_from_latest_state();
        app.dispatch_click(PlaylistDetailWidget::Import);
        app.wait_network().await;
        app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
        app.wait_network().await;
        let entries = app.latest_state().current_storage_entries.unwrap();
        app.dispatch_click(StorageImportWidget::StorageEntry {
            path: entries.entries[4].path.clone(),
        });
        let state = app.latest_state();
        let entries = state.current_storage_entries.unwrap();
        let item = &entries.entries[4];
        assert_eq!(item.name, "angelical-pad-143276.mp3");
        assert_eq!(item.path, "/angelical-pad-143276.mp3");
        assert_eq!(item.is_folder, false);
        assert_eq!(item.can_check, true);
        assert_eq!(item.checked, true);
        assert_eq!(entries.selected_count, 1);

        app.dispatch_click(StorageImportWidget::Import);
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
    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;
    app.dispatch_click(PlaylistDetailWidget::Remove);
    app.wait_network().await;
    create_playlist_and_import_music().await;
    drop(app);

    // reload
    let app = TestApp::new("test-dbs/playlist_full_reimport_discarded_bug", false).await;
    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].count, 1);
}

#[tokio::test]
async fn playlist_full_import_storage_count_bug() {
    let mut app = TestApp::new("test-dbs/playlist_full_import_storage_count_bug", true).await;
    app.setup_preset(PresetDepth::Storage).await;

    app.dispatch_click(PlaylistListWidget::Add);
    app.dispatch_click(PlaylistCreateWidget::Tab {
        value: CreatePlaylistMode::Full,
    });
    app.dispatch_click(PlaylistCreateWidget::Import);
    app.wait_network().await;
    let storage_id = app.get_first_storage_id_from_latest_state();
    app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
    app.wait_network().await;
    let entries = app.latest_state().current_storage_entries.unwrap();
    app.dispatch_click(StorageImportWidget::StorageEntry {
        path: entries.entries[4].path.clone(),
    });
    let entries = app.latest_state().current_storage_entries.unwrap();
    let item = &entries.entries[4];
    assert_eq!(item.name, "angelical-pad-143276.mp3");
    assert_eq!(item.path, "/angelical-pad-143276.mp3");
    assert_eq!(item.is_folder, false);
    assert_eq!(item.can_check, true);
    assert_eq!(item.checked, true);
    assert_eq!(entries.selected_count, 1);
    app.dispatch_click(StorageImportWidget::Import);
    app.wait_network().await;

    app.dispatch_change_text(PlaylistCreateWidget::Name, "ABC");
    app.dispatch_click(PlaylistCreateWidget::FinishCreate);
    app.wait_network().await;
    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    assert_eq!(state.playlist_list[0].title, "ABC");

    app.dispatch_click(StorageListWidget::Item {
        id: app.get_first_storage_id_from_latest_state(),
    });
    let state = app.latest_state().edit_storage.unwrap();
    assert_eq!(state.music_count, 1);
}
