use ease_client_shared::uis::rotuer::VRootSubKeyState;

use crate::RootViewModelState;

use super::service::RouterState;

pub fn root_subkey_view_model(state: &RouterState, root: &mut RootViewModelState) {
    root.current_router = Some(VRootSubKeyState {
        subkey: state.current_sub_route_key.clone(),
    });
}
