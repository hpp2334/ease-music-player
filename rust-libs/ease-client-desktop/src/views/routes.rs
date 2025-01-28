use ease_client::DesktopRoutesKey;
use gpui::{div, prelude::*, View, ViewContext};

use crate::core::view_state::ViewStates;

use super::{playlists::PlaylistListComponent, setting::SettingComponent};

pub struct RoutesComponent {
    vs: ViewStates,
    view_playlist_list: View<PlaylistListComponent>,
    view_setting: View<SettingComponent>
}

impl RoutesComponent {
    pub fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        cx.observe(&vs.route_stack, |_, _, _| {}).detach();
        Self {
            vs: vs.clone(),
            view_playlist_list: cx.new_view(|cx| {
                PlaylistListComponent::new(cx, vs)
            }),
            view_setting: cx.new_view(|cx| {
                SettingComponent::new(cx, vs)
            }),
        }
    }
}

impl Render for RoutesComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let route = self.vs.route_stack.read(cx).current();

        div()
            .size_full()
            .when(route == DesktopRoutesKey::Home, |el| el.child(self.view_playlist_list.clone()))
            .when(route == DesktopRoutesKey::Setting, |el| el.child(self.view_setting.clone()))
    }
}
