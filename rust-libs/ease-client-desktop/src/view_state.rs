use ease_client::{
    view_models::view_state::views::playlist::VPlaylistListState, IViewStateService,
};
use gpui::Model;

pub struct ViewStateService {
    playlist_list: Model<VPlaylistListState>,
}

impl IViewStateService for ViewStateService {
    fn handle_notify(&self, v: ease_client::RootViewModelState) {
        if v.playlist_list.is_some() {}
    }
}
