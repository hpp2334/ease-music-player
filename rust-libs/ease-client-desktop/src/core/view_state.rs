use std::{cell::RefCell, rc::Rc};

use ease_client::{
    view_models::{storage::state::AllStorageState, view_state::views::{playlist::VPlaylistListState, storage::{VEditStorageState, VStorageListState}}}, DesktopRoutesKey, IViewStateService
};
use gpui::{Context, Model};

#[derive(Default, Clone)]
pub struct RouteStack {
    pub routes: Vec<DesktopRoutesKey>,
    pub dirty: bool,
}

#[derive(Clone)]
pub struct ViewStates {
    pub playlist_list: Model<VPlaylistListState>,
    pub storage_list: Model<VStorageListState>,
    pub storage_upsert: Model<VEditStorageState>,
    pub route_stack: Model<RouteStack>,
}

pub struct GpuiViewStateService {
    states: Rc<RefCell<Option<ease_client::RootViewModelState>>>
}

impl RouteStack {
    pub fn current(&self) -> DesktopRoutesKey {
        self.routes.last().cloned().unwrap_or(DesktopRoutesKey::Home)
    }
}

impl ViewStates {
    pub fn new(cx: &mut gpui::AppContext) -> Self {
        Self {
            playlist_list: cx.new_model(|_| VPlaylistListState::default()),
            storage_list: cx.new_model(|_| VStorageListState::default()),
            storage_upsert: cx.new_model(|_| VEditStorageState::default()),
            route_stack: cx.new_model(|_| RouteStack::default())
        }
    }
}

impl GpuiViewStateService {
    pub fn new(states: Rc<RefCell<Option<ease_client::RootViewModelState>>>) -> Self {
        Self {
            states,
        }
    }
}

impl IViewStateService for GpuiViewStateService {
    fn handle_notify(&self, v: ease_client::RootViewModelState) {
        *self.states.borrow_mut() = Some(v);
    }
}
