use ease_client::modules::{controller_update_root_subkey, RootRouteSubKey};
use ease_client_test::{PresetDepth, TestApp};

#[test]
fn router_to_dashboard() {
    let mut app = TestApp::new("test-dbs/router_to_dashboard", true);
    app.setup_preset(PresetDepth::Music);
    app.call_controller(controller_update_root_subkey, RootRouteSubKey::Dashboard);

    let state = app.latest_state();
    let state = state.current_router.unwrap();
    assert_eq!(state.subkey, RootRouteSubKey::Dashboard);
}

#[test]
fn router_to_setting() {
    let mut app = TestApp::new("test-dbs/router_to_setting", true);
    app.setup_preset(PresetDepth::Music);
    app.call_controller(controller_update_root_subkey, RootRouteSubKey::Setting);

    let state = app.latest_state();
    let state = state.current_router.unwrap();
    assert_eq!(state.subkey, RootRouteSubKey::Setting);
}
