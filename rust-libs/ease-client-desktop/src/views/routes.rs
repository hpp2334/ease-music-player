use ease_client::DesktopRoutesKey;
use gpui::{div, prelude::*, Entity};

use crate::core::view_state::ViewStates;

use super::{playlists::PlaylistListComponent, setting::SettingComponent};

pub struct RoutesComponent {
    vs: ViewStates,
    view_playlist_list: Entity<PlaylistListComponent>,
    view_setting: Entity<SettingComponent>,
}

impl RoutesComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        cx.observe(&vs.routes, |_, _, _| {}).detach();
        Self {
            vs: vs.clone(),
            view_playlist_list: cx.new(|cx| PlaylistListComponent::new(cx, vs)),
            view_setting: cx.new(|cx| SettingComponent::new(cx, vs)),
        }
    }
}

impl Render for RoutesComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.vs.routes.read(cx).current();

        div()
            .size_full()
            .when(route == DesktopRoutesKey::Home, |el| {
                el.child(self.view_playlist_list.clone())
            })
            .when(route == DesktopRoutesKey::Setting, |el| {
                el.child(self.view_setting.clone())
            })
    }
}
