use crate::RootViewModelState;

use super::{service::RouterState, VRootSubKeyState};

pub fn root_subkey_view_model(state: &RouterState, root: &mut RootViewModelState) {
    root.current_router = Some(VRootSubKeyState {
        subkey: state.current_sub_route_key.clone(),
    });
}
