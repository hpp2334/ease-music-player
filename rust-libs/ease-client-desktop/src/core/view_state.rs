use gpui::prelude::*;
use ease_client::{
    view_models::{storage::state::AllStorageState, view_state::views::{playlist::VPlaylistListState, storage::VStorageListState}}, DesktopRoutesKey, IViewStateService
};
use gpui::{Context, Model};

#[derive(Default)]
pub struct RouteStack {
    pub routes: Vec<DesktopRoutesKey>
}

#[derive(Clone)]
pub struct ViewStates {
    pub playlist_list: Model<VPlaylistListState>,
    pub storage_list: Model<VStorageListState>,
    pub route_stack: Model<RouteStack>,
}

pub struct GpuiViewStateService {
    cx: gpui::AsyncAppContext,
    states: ViewStates,
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
            route_stack: cx.new_model(|_| RouteStack::default())
        }
    }
}

impl GpuiViewStateService {
    pub fn new(cx: &mut gpui::AppContext, states: ViewStates) -> Self {
        Self {
            cx: cx.to_async(),
            states,
        }
    }
}

impl IViewStateService for GpuiViewStateService {
    fn handle_notify(&self, v: ease_client::RootViewModelState) {
        let u = v.playlist_list.clone();
        if u.is_some() {
            let state = self.states.playlist_list.clone();
            self.cx
                .update(move |cx| {
                    state.update(cx, |v, _| {
                        *v = u.unwrap();
                    })
                })
                .unwrap();
        }
    }
}
