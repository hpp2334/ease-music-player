use std::time::Duration;

use ease_client::modules::*;
use ease_client_test::{PresetDepth, TestApp};

#[test]
fn music_play_1() {
    let mut app = TestApp::new("test-dbs/music_play_1", true);
    app.setup_preset(PresetDepth::Music);

    let music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, music_id);
    app.advance_timer(2);
    let state = app.latest_state();
    assert_eq!(state.current_music.as_ref().unwrap().playing, true);

    app.call_controller(controller_pause_music, ());
    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");
    app.call_controller(controller_resume_music, ());

    app.advance_timer(2);
    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:04");

    app.call_controller(controller_seek_music, ArgSeekMusic { duration: 2000 });
    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);
    let state = state.current_music.clone().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    app.call_controller(controller_pause_music, ());
}

#[test]
fn music_play_2() {
    let mut app = TestApp::new("test-dbs/music_play_2", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);
    app.advance_timer(1);
    app.call_controller(controller_pause_music, ());
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:01");
    assert_eq!(state.total_duration, "00:00:24");
    let state = app.latest_state();
    let state = state.current_playlist.as_ref().unwrap();
    assert_eq!(state.items[0].duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.call_controller(controller_play_music, b_music_id);
    app.advance_timer(3);
    app.call_controller(controller_pause_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:03");
    assert_eq!(state.total_duration, "00:00:06");

    app.call_controller(controller_resume_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);

    app.advance_timer(1);
    app.call_controller(controller_pause_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.current_duration, "00:00:04");
    assert_eq!(state.total_duration, "00:00:06");

    app.call_controller(controller_pause_music, ());
}

#[test]
fn music_play_3() {
    let mut app = TestApp::new("test-dbs/music_play_3", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.call_controller(controller_play_music, b_music_id);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.total_duration, "00:00:06");
    app.call_controller(controller_pause_music, ());
}

#[test]
fn music_play_4() {
    let mut app = TestApp::new("test-dbs/music_play_4", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.call_controller(controller_play_music, b_music_id);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.total_duration, "00:00:06");
    app.call_controller(controller_stop_music, ());
}

#[test]
fn music_play_5() {
    let mut app = TestApp::new("test-dbs/music_play_5", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");

    let b_music_id = app.get_second_music_id_from_latest_state();
    app.call_controller(controller_play_music, b_music_id);
    app.advance_timer(4);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:04");
    assert_eq!(state.total_duration, "00:00:06");
    app.call_controller(controller_stop_music, ());

    let state = app.latest_state();
    let state = state.current_playlist.unwrap();
    assert_eq!(state.duration, "00:00:30");
}

#[test]
fn music_play_6() {
    let mut app = TestApp::new("test-dbs/music_play_6", true);
    app.setup_preset(PresetDepth::Music);

    app.call_controller(controller_play_all_musics, ());
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.current_duration, "00:00:02");
    assert_eq!(state.total_duration, "00:00:24");
}

#[test]
fn music_play_single_non_loop() {
    let mut app = TestApp::new("test-dbs/music_play_single_non_loop", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);

    app.call_controller(
        controller_seek_music,
        ArgSeekMusic {
            duration: Duration::from_secs(23).as_millis() as u64,
        },
    );
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, false);
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.current_duration_ms, 0);

    app.call_controller(controller_play_next_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "b");
    assert_eq!(state.playing, true);
    let state = app.latest_state().current_music_lyric.unwrap();
    assert_eq!(state.load_state, LyricLoadState::Missing);

    app.advance_timer(1);
    app.call_controller(controller_play_previous_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.playing, true);
    app.call_controller(controller_pause_music, ());
}

#[test]
fn music_play_list_non_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_list_non_loop_1", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_play_music, a_music_id);
    app.call_controller(
        controller_seek_music,
        ArgSeekMusic {
            duration: Duration::from_secs(23).as_millis() as u64,
        },
    );
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::List);

    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "b");
    assert_eq!(state.total_duration, "00:00:06");
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, false);
}

#[test]
fn music_play_list_non_loop_to_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_list_non_loop_to_loop_1", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_play_music, a_music_id);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.can_play_previous, false);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::List);

    app.call_controller(controller_update_music_playmode_to_next, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::ListLoop);
}

#[test]
fn music_play_single_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_single_loop_1", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_play_music, a_music_id);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, true);
    assert_eq!(state.play_mode, PlayMode::SingleLoop);

    app.call_controller(
        controller_seek_music,
        ArgSeekMusic {
            duration: Duration::from_secs(23).as_millis() as u64,
        },
    );
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:01");
    assert_eq!(state.can_play_previous, true);
    assert_eq!(state.can_play_next, true);

    app.call_controller(controller_play_next_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "b");
    assert_eq!(state.playing, true);

    app.advance_timer(1);
    app.call_controller(controller_play_previous_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.playing, true);
    app.call_controller(controller_pause_music, ());
}

#[test]
fn music_play_list_loop_1() {
    let mut app = TestApp::new("test-dbs/music_play_list_loop_1", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_update_music_playmode_to_next, ());
    app.call_controller(controller_play_music, a_music_id);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:00");
    assert_eq!(state.play_mode, PlayMode::ListLoop);

    app.call_controller(
        controller_seek_music,
        ArgSeekMusic {
            duration: Duration::from_secs(23).as_millis() as u64,
        },
    );
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "b");
    assert_eq!(state.current_duration, "00:00:01");

    app.call_controller(
        controller_seek_music,
        ArgSeekMusic {
            duration: Duration::from_secs(23).as_millis() as u64,
        },
    );
    app.advance_timer(2);
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:01");

    app.call_controller(controller_play_next_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "b");
    assert_eq!(state.current_duration, "00:00:00");

    app.call_controller(controller_play_previous_music, ());
    let state = app.latest_state();
    let state = state.current_music.as_ref().unwrap();
    assert_eq!(state.playing, true);
    assert_eq!(state.title, "angelical-pad-143276");
    assert_eq!(state.current_duration, "00:00:00");

    app.call_controller(controller_pause_music, ());
}

#[test]
fn music_play_buf() {
    let mut app = TestApp::new("test-dbs/music_play_buf", true);
    app.setup_preset(PresetDepth::Music);

    let music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, music_id);
    app.advance_timer(1);
    app.call_controller(controller_pause_music, ());

    let state = app.latest_state();
    assert_eq!(state.current_music.is_some(), true);

    let bytes = app.get_lastest_bytes()[0..10].to_vec();
    assert_eq!(bytes, &[73, 68, 51, 3, 0, 0, 0, 0, 119, 118]);

    app.call_controller(controller_pause_music, ());
}

#[test]
fn test_music_import_repeated() {
    let mut app = TestApp::new("test-dbs/test_music_import_repeated", true);
    app.setup_preset(PresetDepth::Playlist);
    let playlist_id = app.get_first_playlist_id_from_latest_state();
    app.call_controller(controller_change_current_playlist, playlist_id);

    let import_entries = || {
        let storage_id = app.get_first_storage_id_from_latest_state();
        app.call_controller(controller_prepare_import_entries_in_current_playlist, ());
        app.call_controller(controller_select_storage_in_import, storage_id);
        let state = app.latest_state();
        let entries = state.current_storage_entries.unwrap();
        app.call_controller(controller_select_entry, entries.entries[4].path.clone());
        app.call_controller(controller_select_entry, entries.entries[5].path.clone());
        app.call_controller(controller_finish_selected_entries_in_import, ());
    };
    import_entries();
    import_entries();
    let state = app.latest_state();
    let current_playlist = state.current_playlist.unwrap();
    assert_eq!(current_playlist.items.len(), 2);
}

#[test]
fn remove_current_playing_playlist_when_playing_music() {
    let mut app: TestApp = TestApp::new(
        "test-dbs/remove_current_playing_playlist_when_playing_music",
        true,
    );
    app.setup_preset(PresetDepth::Music);

    let playlist_id = app.get_first_playlist_id_from_latest_state();
    let music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, music_id);
    app.advance_timer(1);

    app.call_controller(controller_remove_playlist, playlist_id);
    let state = app.latest_state();
    let state = state.current_music.unwrap();
    assert!(state.id.is_none());
}

#[test]
fn remove_current_playing_music_when_playing_music() {
    let mut app: TestApp = TestApp::new(
        "test-dbs/remove_current_playing_music_when_playing_music",
        true,
    );
    app.setup_preset(PresetDepth::Music);

    let music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, music_id.clone());
    app.advance_timer(1);

    app.call_controller(controller_remove_music_from_current_playlist, music_id);
    let state = app.latest_state();
    let state = state.current_music.unwrap();
    assert!(state.id.is_none());
}

#[test]
fn time_to_pause_1() {
    let mut app: TestApp = TestApp::new("test-dbs/time_to_pause_1", true);
    app.setup_preset(PresetDepth::Music);

    let music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, music_id.clone());

    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);

    app.call_controller(controller_update_time_to_pause, 1_000);
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, true);
    app.advance_timer(2);
    app.wait_network();
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, false);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, false);
}

#[test]
fn time_to_pause_2() {
    let mut app: TestApp = TestApp::new("test-dbs/time_to_pause_2", true);
    app.setup_preset(PresetDepth::Music);

    let music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, music_id.clone());

    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);

    app.call_controller(controller_update_time_to_pause, 100_000);
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, true);
    assert_eq!(state.left_hour, 0);
    assert_eq!(state.left_minute, 1);

    app.call_controller(controller_remove_time_to_pause, ());
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.playing, true);
    let state = app.latest_state().time_to_pause.unwrap();
    assert_eq!(state.enabled, false);
}

#[test]
fn music_cover_1() {
    let mut app = TestApp::new("test-dbs/music_cover_1", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);
    let state = app.latest_state().current_music.unwrap();
    let picture = app.load_resource(state.cover);
    assert_eq!(picture.len(), 15025);

    let state = app.latest_state().playlist_list.unwrap();
    assert_eq!(state.playlist_list.len(), 1);
    let picture = app.load_resource(*state.playlist_list[0].picture.as_ref().unwrap());
    assert_eq!(picture.len(), 15025);

    let state = app.latest_state().current_playlist.unwrap();
    let picture = app.load_resource(*state.picture.as_ref().unwrap());
    assert_eq!(picture.len(), 15025);
}
