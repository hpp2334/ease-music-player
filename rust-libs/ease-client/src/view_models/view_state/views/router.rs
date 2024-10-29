
use serde::Serialize;

use crate::view_models::main::state::{RootRouteSubKey, RouterState};

use super::models::RootViewModelState;



#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VRootSubKeyState {
    pub subkey: RootRouteSubKey,
}

pub(crate) fn root_subkey_vs(state: &RouterState, root: &mut RootViewModelState) {
    root.current_router = Some(VRootSubKeyState {
        subkey: state.subkey.clone(),
    });
}
