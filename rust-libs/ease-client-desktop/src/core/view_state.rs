use std::fmt::Debug;

use ease_client::{
    view_models::view_state::views::{
        playlist::{VCreatePlaylistState, VPlaylistListState},
        storage::{VEditStorageState, VStorageListState},
    },
    IViewStateService,
};
use gpui::{AppContext, Entity};

use crate::utils::dynamic_lifetime::SharedDynamicLifetime;

use super::routes::Router;

#[derive(Clone)]
pub struct ViewStates {
    pub playlist_list: Entity<VPlaylistListState>,
    pub playlist_create: Entity<VCreatePlaylistState>,
    pub storage_list: Entity<VStorageListState>,
    pub storage_upsert: Entity<VEditStorageState>,
    pub router: Entity<Router>,
}

pub struct GpuiViewStateService {
    gpui_vs: ViewStates,
    dyn_app: SharedDynamicLifetime<gpui::App>,
}

impl ViewStates {
    pub fn new(cx: &mut gpui::App) -> Self {
        Self {
            playlist_list: cx.new(|_| Default::default()),
            playlist_create: cx.new(|_| Default::default()),
            storage_list: cx.new(|_| Default::default()),
            storage_upsert: cx.new(|_| Default::default()),
            router: cx.new(|_| Router::new()),
        }
    }
}

impl GpuiViewStateService {
    pub fn new(vs: ViewStates, dyn_app: SharedDynamicLifetime<gpui::App>) -> Self {
        Self {
            gpui_vs: vs,
            dyn_app,
        }
    }

    fn flush_impl(&self, cx: &mut gpui::App, v: ease_client::RootViewModelState) {
        self.flush_vs(cx, &v.playlist_list, &self.gpui_vs.playlist_list);
        self.flush_vs(cx, &v.create_playlist, &self.gpui_vs.playlist_create);
        self.flush_vs(cx, &v.storage_list, &self.gpui_vs.storage_list);
        self.flush_vs(cx, &v.edit_storage, &self.gpui_vs.storage_upsert);
    }

    fn flush_vs<C, V>(&self, cx: &mut C, vs: &Option<V>, m: &Entity<V>)
    where
        C: gpui::AppContext,
        V: Debug + Clone + 'static,
    {
        let u = vs.clone();
        if u.is_some() {
            let state = m.clone();
            state.update(cx, |v, cx| {
                *v = u.unwrap();
                cx.notify();
            });
        }
    }
}

impl IViewStateService for GpuiViewStateService {
    fn handle_notify(&self, v: ease_client::RootViewModelState) {
        let mut app = self.dyn_app.get();
        let cx = app.get();
        self.flush_impl(cx, v);
    }
}
