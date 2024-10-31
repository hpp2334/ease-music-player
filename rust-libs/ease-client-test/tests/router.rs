use ease_client::{view_models::main::state::RootRouteSubKey, MainBodyWidget};
use ease_client_test::{PresetDepth, TestApp};

#[tokio::test]
async fn router_to_dashboard() {
    let mut app = TestApp::new("test-dbs/router_to_dashboard", true).await;
    app.setup_preset(PresetDepth::Music).await;
    app.dispatch_click(MainBodyWidget::Tab {
        key: RootRouteSubKey::Dashboard,
    });

    let state = app.latest_state();
    let state = state.current_router.unwrap();
    assert_eq!(state.subkey, RootRouteSubKey::Dashboard);
}

#[tokio::test]
async fn router_to_setting() {
    let mut app = TestApp::new("test-dbs/router_to_setting", true).await;
    app.setup_preset(PresetDepth::Music).await;
    app.dispatch_click(MainBodyWidget::Tab {
        key: RootRouteSubKey::Setting,
    });

    let state = app.latest_state();
    let state = state.current_router.unwrap();
    assert_eq!(state.subkey, RootRouteSubKey::Setting);
}
