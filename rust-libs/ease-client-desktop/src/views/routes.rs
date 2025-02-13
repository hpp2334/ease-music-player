use ease_client::DesktopRoutesKey;
use gpui::{div, prelude::*, Entity};

use crate::core::view_state::ViewStates;

use super::{
    playlist::PlaylistComponent, playlists::PlaylistListComponent, setting::SettingComponent,
};

pub struct RoutesComponent {
    vs: ViewStates,
    view_playlist_list: Entity<PlaylistListComponent>,
    view_playlist: Entity<PlaylistComponent>,
    view_setting: Entity<SettingComponent>,
}

impl RoutesComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        cx.observe(&vs.router, |_, _, _| {}).detach();
        Self {
            vs: vs.clone(),
            view_playlist_list: cx.new(|cx| PlaylistListComponent::new(cx, vs)),
            view_playlist: cx.new(|cx| PlaylistComponent::new(cx, vs)),
            view_setting: cx.new(|cx| SettingComponent::new(cx, vs)),
        }
    }
}

impl Render for RoutesComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.vs.router.read(cx).current();

        div()
            .size_full()
            .when(route == DesktopRoutesKey::Home, |el| {
                el.child(self.view_playlist_list.clone())
            })
            .when(route == DesktopRoutesKey::Playlist, |el| {
                el.child(self.view_playlist.clone())
            })
            .when(route == DesktopRoutesKey::Setting, |el| {
                el.child(self.view_setting.clone())
            })
    }
}
