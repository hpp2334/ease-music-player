

use ease_client::modules::*;
use ease_client_test::{PresetDepth, TestApp};

#[test]
fn music_lyric_1() {
    let mut app = TestApp::new("test-dbs/music_lyric_1", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    let state = app.latest_state().current_playlist.unwrap();
    assert_eq!(state.duration, "00:00:30");

    app.call_controller(controller_play_music, a_music_id);
    let state = app.latest_state().current_music_lyric.unwrap();
    let lines = state.lyric_lines;
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], (4070, "This is the first line".to_string()));
    assert_eq!(lines[1], (7110, "This is the second line".to_string()));
    assert_eq!(lines[2], (8910, "This is the third line".to_string()));
    assert_eq!(lines[3], (19310, "This is the last line".to_string()));
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.lyric_index, -1);

    app.advance_timer(5);
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.lyric_index, 0);

    app.advance_timer(3);
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.lyric_index, 1);

    app.advance_timer(2);
    let state = app.latest_state().current_music.unwrap();
    assert_eq!(state.lyric_index, 2);
}

#[test]
fn music_lyric_2() {
    let mut app = TestApp::new("test-dbs/music_lyric_2", true);
    app.setup_preset(PresetDepth::Music);

    let a_music_id = app.get_first_music_id_from_latest_state();
    app.call_controller(controller_play_music, a_music_id);

    app.wait_network();
    let state = app.latest_state().current_music_lyric.unwrap();
    let lines = state.lyric_lines;
    assert_eq!(lines.len(), 4);
    assert_eq!(state.load_state, LyricLoadState::Loaded);

    app.call_controller(controller_remove_current_music_lyric, ());
    let state = app.latest_state().current_music_lyric.unwrap();
    let lines = state.lyric_lines;
    assert_eq!(lines.len(), 0);
    assert_eq!(state.load_state, LyricLoadState::Missing);

    app.call_controller(controller_prepare_import_lyric, ());
    app.wait_network();

    let state = app.latest_state().current_storage_entries.unwrap();
    let entry = state.entries[3].clone();
    assert_eq!(entry.name, "angelical-pad-143276.lrc");
    assert_eq!(entry.can_check, true);

    app.call_controller(controller_select_entry, entry.path);
    app.call_controller(controller_finish_selected_entries_in_import, ());
    app.wait_network();
    let state = app.latest_state().current_music_lyric.unwrap();
    let lines = state.lyric_lines;
    assert_eq!(lines.len(), 4);
    assert_eq!(state.load_state, LyricLoadState::Loaded);
}
