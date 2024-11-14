use std::time::Duration;

use ease_client::{
    view_models::*, Action, MusicControlWidget, PlaylistDetailWidget, PlaylistListWidget,
    StorageImportWidget, TimeToPauseWidget, ViewAction,
};
use ease_client_shared::{backends::music::LyricLoadState, uis::preference::PlayMode};
use ease_client_test::{PresetDepth, TestApp};
use music::{control::MusicControlAction, time_to_pause::TimeToPauseAction};

#[tokio::test]
async fn music_play_1() {
    let mut app = TestApp::new("test-dbs/music_play_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    app.dispatch_click(PlaylistListWidget::Item {
        id: app.get_first_playlist_id_from_latest_state(),
    });
    app.wait_network().await;
    app.dispatch_click(PlaylistDetailWidget::Music {
        id: app.get_first_music_id_from_latest_state(),
    });
    app.wait_network().await;
    app.advance_timer(2).await;
    let state = app.latest_state();
    assert_eq!(state.current_music.as_ref().unwrap().playing, true);

    app.dispatch_click(MusicControlWidget::Pause);
    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");
    app.dispatch_click(MusicControlWidget::Play);

    app.advance_timer(2).await;
    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:04");

    app.emit(Action::View(ViewAction::MusicControl(
        MusicControlAction::Seek { duration_ms: 2000 },
    )));
    app.wait_network().await;
    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn music_play_2() {
    let mut app = TestApp::new("test-dbs/music_play_2", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.advance_timer(1).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:01");

    app.dispatch_click(MusicControlWidget::Pause);
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:01");
    assert_eq!(state.total_duration, "00:00:24");
    let state = app.latest_state();
    let state = state.current_playlist.as_ref().unwrap();
    assert_eq!(state.items[0].duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: b_music_id });
    app.wait_network().await;
    app.advance_timer(3).await;
    app.dispatch_click(MusicControlWidget::Pause);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:03");
    assert_eq!(state.total_duration, "00:00:06");

    app.dispatch_click(MusicControlWidget::Play);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);

    app.advance_timer(1).await;
    app.dispatch_click(MusicControlWidget::Pause);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:04");
    assert_eq!(state.total_duration, "00:00:06");

    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn music_play_3() {
    let mut app = TestApp::new("test-dbs/music_play_3", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: b_music_id });
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.total_duration, "00:00:06");
    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn music_play_4() {
    let mut app = TestApp::new("test-dbs/music_play_4", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: b_music_id });
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.total_duration, "00:00:06");
    app.dispatch_click(MusicControlWidget::Stop);
    app.wait_network().await;
}

#[tokio::test]
async fn music_play_5() {
    let mut app = TestApp::new("test-dbs/music_play_5", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: b_music_id });
    app.wait_network().await;
    app.advance_timer(4).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:04");
    assert_eq!(state.total_duration, "00:00:06");
    app.dispatch_click(MusicControlWidget::Stop);
    app.wait_network().await;

    let state = app.latest_state();
    let state = state.current_playlist.unwrap();
    assert_eq!(state.duration, "00:00:30");
}

#[tokio::test]
async fn music_play_6() {
    let mut app = TestApp::new("test-dbs/music_play_6", true).await;
    app.setup_preset(PresetDepth::Music).await;

    app.dispatch_click(PlaylistDetailWidget::PlayAll);
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");
}

#[tokio::test]
async fn music_play_single_non_loop() {
    let mut app = TestApp::new("test-dbs/music_play_single_non_loop", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::Single);

    app.emit(Action::View(ViewAction::MusicControl(
        MusicControlAction::Seek {
            duration_ms: Duration::from_secs(23).as_millis() as u64,
        },
    )));
    app.wait_network().await;
    app.advance_timer(2).await;

    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.current_duration_ms, 0);

    app.dispatch_click(MusicControlWidget::PlayNext);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "b");
    assert_eq!(state.playing, true);
    let state = app.latest_state().current_music_lyric.unwrap();
    assert_eq!(state.load_state, LyricLoadState::Missing);

    app.advance_timer(1).await;
    app.dispatch_click(MusicControlWidget::PlayPrevious);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.playing, true);
    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn music_play_list_non_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_list_non_loop_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.dispatch_click(MusicControlWidget::Playmode);
    app.dispatch_click(MusicControlWidget::Playmode);
    app.emit(Action::View(ViewAction::MusicControl(
        MusicControlAction::Seek {
            duration_ms: Duration::from_secs(23).as_millis() as u64,
        },
    )));
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.play_mode, PlayMode::List);
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);

    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "b");
    assert_eq!(state.total_duration, "00:00:06");
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, false);
}

#[tokio::test]
async fn music_play_list_non_loop_to_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_list_non_loop_to_loop_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.dispatch_click(MusicControlWidget::Playmode);
    app.dispatch_click(MusicControlWidget::Playmode);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::List);

    app.dispatch_click(MusicControlWidget::Playmode);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::ListLoop);
}

#[tokio::test]
async fn music_play_single_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_single_loop_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.dispatch_click(MusicControlWidget::Playmode);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::SingleLoop);

    app.emit(Action::View(ViewAction::MusicControl(
        MusicControlAction::Seek {
            duration_ms: Duration::from_secs(23).as_millis() as u64,
        },
    )));
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:01");
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, true);

    app.dispatch_click(MusicControlWidget::PlayNext);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "b");
    assert_eq!(state.playing, true);

    app.advance_timer(1).await;
    app.dispatch_click(MusicControlWidget::PlayPrevious);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.playing, true);
    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn music_play_list_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_list_loop_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
    app.wait_network().await;
    app.dispatch_click(MusicControlWidget::Playmode);
    app.dispatch_click(MusicControlWidget::Playmode);
    app.dispatch_click(MusicControlWidget::Playmode);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.play_mode, PlayMode::ListLoop);

    app.emit(Action::View(ViewAction::MusicControl(
        MusicControlAction::Seek {
            duration_ms: Duration::from_secs(23).as_millis() as u64,
        },
    )));
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "b");
    assert_eq!(state.current_duration, "00:00:01");

    app.emit(Action::View(ViewAction::MusicControl(
        MusicControlAction::Seek {
            duration_ms: Duration::from_secs(5).as_millis() as u64,
        },
    )));
    app.advance_timer(2).await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:01");

    app.dispatch_click(MusicControlWidget::PlayNext);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "b");
    assert_eq!(state.current_duration, "00:00:00");

    app.dispatch_click(MusicControlWidget::PlayPrevious);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:00");

    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn music_play_buf() {
    let mut app = TestApp::new("test-dbs/music_play_buf", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music { id: music_id });
    app.advance_timer(1).await;
    app.dispatch_click(MusicControlWidget::Pause);

    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);

    let bytes = app.get_lastest_bytes()[0..10].to_vec();
    assert_eq!(bytes, &[73, 68, 51, 3, 0, 0, 0, 0, 119, 118]);

    app.dispatch_click(MusicControlWidget::Pause);
}

#[tokio::test]
async fn test_music_import_repeated() {
    let mut app = TestApp::new("test-dbs/test_music_import_repeated", true).await;
    app.setup_preset(PresetDepth::Playlist).await;
    let playlist_id = app.get_first_playlist_id_from_latest_state();
    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;

    let import_entries = || async {
        let storage_id = app.get_first_storage_id_from_latest_state();
        app.dispatch_click(PlaylistDetailWidget::Import);
        app.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
        app.wait_network().await;
        let state = app.latest_state();
        let entries = state.current_storage_entries.unwrap();
        app.dispatch_click(StorageImportWidget::StorageEntry {
            path: entries.entries[4].path.clone(),
        });
        app.dispatch_click(StorageImportWidget::StorageEntry {
            path: entries.entries[5].path.clone(),
        });
        app.dispatch_click(StorageImportWidget::Import);
        app.wait_network().await;
    };
    import_entries().await;
    import_entries().await;
    let state = app.latest_state();
    let current_playlist = state.current_playlist.unwrap();
    assert_eq!(current_playlist.items.len(), 2);
}

#[tokio::test]
async fn remove_current_playing_playlist_when_playing_music() {
    let mut app: TestApp = TestApp::new(
        "test-dbs/remove_current_playing_playlist_when_playing_music",
        true,
    )
    .await;
    app.setup_preset(PresetDepth::Music).await;

    let playlist_id = app.get_first_playlist_id_from_latest_state();
    let music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
    app.wait_network().await;
    app.dispatch_click(PlaylistDetailWidget::Music { id: music_id });
    app.advance_timer(1).await;

    app.dispatch_click(PlaylistDetailWidget::Remove);
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.unwrap();
    assert!(state.id.is_none());
}

#[tokio::test]
async fn remove_current_playing_music_when_playing_music() {
    let mut app: TestApp = TestApp::new(
        "test-dbs/remove_current_playing_music_when_playing_music",
        true,
    )
    .await;
    app.setup_preset(PresetDepth::Music).await;

    let music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music {
        id: music_id.clone(),
    });
    app.advance_timer(1).await;

    app.dispatch_click(PlaylistDetailWidget::RemoveMusic { id: music_id });
    app.wait_network().await;
    let state = app.latest_state();
    let state = state.current_music.unwrap();
    assert!(state.id.is_none());
}

#[tokio::test]
async fn time_to_pause_1() {
    let mut app: TestApp = TestApp::new("test-dbs/time_to_pause_1", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music {
        id: music_id.clone(),
    });
    app.wait_network().await;

    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);

    app.emit(Action::View(ViewAction::TimeToPause(
        TimeToPauseAction::Finish {
            hour: 0,
            minute: 0,
            second: 3,
        },
    )));
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, true);
    app.advance_timer(4).await;
    app.wait_network().await;
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, false);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, false);
}

#[tokio::test]
async fn time_to_pause_2() {
    let mut app: TestApp = TestApp::new("test-dbs/time_to_pause_2", true).await;
    app.setup_preset(PresetDepth::Music).await;

    let music_id = app.get_first_music_id_from_latest_state();
    app.dispatch_click(PlaylistDetailWidget::Music {
        id: music_id.clone(),
    });
    app.wait_network().await;

    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);

    app.emit(Action::View(ViewAction::TimeToPause(
        TimeToPauseAction::Finish {
            hour: 0,
            minute: 2,
            second: 0,
        },
    )));
    app.wait_network().await;
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, true);
    assert_eq!(state.left_hour, 0);
    assert_eq!(state.left_minute, 2);

    app.dispatch_click(TimeToPauseWidget::Delete);
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, false);
}

#[tokio::test]
async fn time_to_pause_3() {
    let mut app: TestApp = TestApp::new("test-dbs/time_to_pause_3", true).await;
    app.setup_preset(PresetDepth::Music).await;

    app.emit(Action::View(ViewAction::TimeToPause(
        TimeToPauseAction::Finish {
            hour: 0,
            minute: 0,
            second: 8,
        },
    )));
    app.advance_timer(8).await;
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, false);
    assert_eq!(state.left_hour, 0);
    assert_eq!(state.left_minute, 0);
}

// #[tokio::test]
// async fn music_cover_1() {
//     let mut app = TestApp::new("test-dbs/music_cover_1", true).await;
//     app.setup_preset(PresetDepth::Music).await;

//     let a_music_id = app.get_first_music_id_from_latest_state();
//     app.dispatch_click(PlaylistDetailWidget::Music { id: a_music_id });
//     app.wait_network().await;
//     let state = app.latest_state().current_music.unwrap();
//     let cover_url = state.cover.clone();
//     let picture = app.load_resource(&cover_url).await;
//     assert_eq!(picture.len(), 15025);

//     let state = app.latest_state().playlist_list.unwrap();
//     assert_eq!(state.playlist_list.len(), 1);
//     let cover_url = state.playlist_list[0].cover_url.clone();
//     let picture = app.load_resource(&cover_url).await;
//     assert_eq!(picture.len(), 15025);

//     let state = app.latest_state().current_playlist.unwrap();
//     let cover_url = state.cover_url.clone();
//     let picture = app.load_resource(&cover_url).await;
//     assert_eq!(picture.len(), 15025);
// }
