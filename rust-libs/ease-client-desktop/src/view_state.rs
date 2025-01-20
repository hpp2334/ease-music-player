use ease_client::{
    view_models::view_state::views::playlist::VPlaylistListState, IViewStateService,
};
use gpui::{Context, Model};

#[derive(Clone)]
pub struct ViewStates {
    playlist_list: Model<VPlaylistListState>,
}

pub struct GpuiViewStateService {
    cx: gpui::AsyncAppContext,
    states: ViewStates,
}

impl GpuiViewStateService {
    pub fn new(cx: &mut gpui::AppContext) -> Self {
        Self {
            cx: cx.to_async(),
            states: ViewStates {
                playlist_list: cx.new_model(|_| VPlaylistListState::default()),
            },
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
