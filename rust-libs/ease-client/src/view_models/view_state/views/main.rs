use serde::Serialize;

use crate::view_models::main::state::{MainState, RootRouteSubKey};

use super::models::RootViewModelState;

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VMainState {
    pub subkey: RootRouteSubKey,
    pub vs_loaded: bool,
}

pub(crate) fn main_vs(state: &MainState, root: &mut RootViewModelState) {
    root.current_router = Some(VMainState {
        subkey: state.subkey.clone(),
        vs_loaded: state.vs_loaded,
    });
}
