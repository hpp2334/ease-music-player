use misty_vm::misty_to_host;

use crate::view_models::view_state::views::models::RootViewModelState;

pub trait IViewStateService: Send + Sync + 'static {
    fn handle_notify(&self, v: RootViewModelState);
}
misty_to_host!(ViewStateService, IViewStateService);
