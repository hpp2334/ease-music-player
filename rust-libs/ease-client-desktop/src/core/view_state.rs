use std::{cell::RefCell, rc::Rc};

use ease_client::{
    view_models::{
        storage::state::AllStorageState,
        view_state::views::{
            playlist::VPlaylistListState,
            storage::{VEditStorageState, VStorageListState},
        },
    },
    DesktopRoutesKey, IViewStateService,
};
use gpui::{AppContext, Context, Entity};

use super::routes::{Router, Routes};

#[derive(Clone)]
pub struct ViewStates {
    pub playlist_list: Entity<VPlaylistListState>,
    pub storage_list: Entity<VStorageListState>,
    pub storage_upsert: Entity<VEditStorageState>,
    pub routes: Entity<Routes>,
}

pub struct GpuiViewStateService {
    states: Rc<RefCell<Option<ease_client::RootViewModelState>>>,
}

impl ViewStates {
    pub fn new(cx: &mut gpui::App) -> Self {
        Self {
            playlist_list: cx.new(|_| VPlaylistListState::default()),
            storage_list: cx.new(|_| VStorageListState::default()),
            storage_upsert: cx.new(|_| VEditStorageState::default()),
            routes: cx.new(|_| Routes::new()),
        }
    }
}

impl GpuiViewStateService {
    pub fn new(states: Rc<RefCell<Option<ease_client::RootViewModelState>>>) -> Self {
        Self { states }
    }
}

impl IViewStateService for GpuiViewStateService {
    fn handle_notify(&self, v: ease_client::RootViewModelState) {
        *self.states.borrow_mut() = Some(v);
    }
}
