use misty_vm::{client::MistyClientHandle, states::MistyStateTrait};

use super::RootRouteSubKey;

#[derive(Debug, Default)]
pub struct RouterState {
    pub current_sub_route_key: RootRouteSubKey,
}

pub fn update_root_subkey(app: MistyClientHandle, arg: RootRouteSubKey) {
    RouterState::update(app, |state| {
        state.current_sub_route_key = arg;
    })
}
