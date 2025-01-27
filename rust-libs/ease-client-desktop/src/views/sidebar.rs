use ease_client::{view_models::{main::sidebar::SidebarWidget, view_state::views::playlist::VPlaylistListState}, DesktopRoutesKey, WidgetAction, WidgetActionType};
use gpui::{div, prelude::*, px, rgb, svg, Model, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY, RGB_PRIMARY_TEXT, RGB_SECONDARY_TEXT, RGB_SLIGHT_100}, view_state::{RouteStack, ViewStates}, vm::AppPodProxy};

fn route_id(key: DesktopRoutesKey) -> &'static str {
    match key {
        DesktopRoutesKey::Home => "route-home",
        DesktopRoutesKey::Setting => "route-setting",
    }
}

pub struct SiderbarHeaderComponent {
    route_stack: Model<RouteStack>,
    icon_path: &'static str,
    text: &'static str,
    route: DesktopRoutesKey,
}

impl Render for SiderbarHeaderComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let active = self.route_stack.read(cx).current() == self.route;

        div()
            .id(SharedString::new_static(route_id(self.route.clone())))
            .w_full()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .cursor_pointer()
            .on_click(move |e, cx| {
                if active {
                    cx.global::<AppPodProxy>().dispatch(WidgetAction {
                        widget: SidebarWidget::Playlists.into(),
                        typ: WidgetActionType::Click,
                    });
                }
            })
            .child(div().w(px(20.0)).h_full())
            .child(
                svg()
                    .size(px(16.0))
                    .text_color(rgb(RGB_PRIMARY_TEXT))
                    .path(self.icon_path)
            )
            .child(div().w(px(6.0)).h_full())
            .child(div()
                .flex_grow()
                .text_color(if active {
                    rgb(RGB_PRIMARY_TEXT)
                } else {
                    rgb(RGB_SECONDARY_TEXT)
                })
                .child(self.text)
            )
            .child(div()
                .w(px(4.0))
                .h_full()
            )
            .child(
                div()
                    .w(px(2.0))
                    .h(px(16.0))
                    .when(active, |el| {
                        el.child(div()
                            .size_full()
                            .rounded_xl()
                            .bg(rgb(RGB_PRIMARY))
                        )
                    })
            )
    }
}
pub struct SidebarComponent {
    playlist_list: Model<VPlaylistListState>,
    route_stack: Model<RouteStack>,
    view_playlist_header: View<SiderbarHeaderComponent>,
    view_setting_header: View<SiderbarHeaderComponent>,
} 

impl SidebarComponent {
    pub fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        let playlist_list = vs.playlist_list.clone();
        cx.observe(&playlist_list, |_, _, _| {}).detach();
        Self {
            playlist_list: vs.playlist_list.clone(),
            route_stack: vs.route_stack.clone(),
            view_playlist_header: cx.new_view(|_| SiderbarHeaderComponent {
                route_stack: vs.route_stack.clone(),
                icon_path: "drawables://AlbumOutline.svg",
                text: "Playlists",
                route: DesktopRoutesKey::Home,
            }),
            view_setting_header: cx.new_view(|_| SiderbarHeaderComponent {
                route_stack: vs.route_stack.clone(),
                icon_path: "drawables://SettingOutline.svg",
                text: "Setting",
                route: DesktopRoutesKey::Setting,
            })
        }
    }
}

impl Render for SidebarComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.playlist_list.read(cx);

        let playlist_elements: Vec<_> = state
            .playlist_list
            .clone()
            .into_iter()
            .map(|item| {
                div()
                    .id(*item.id.as_ref() as usize)
                    .px_2()
                    .cursor_pointer()
                    .on_click({
                        let item = item.clone();
                        move |_event, cx| {
                            println!("VPlaylistAbstractItem {:?}", item);
                        }
                    })
                    .w_full()
                    .h_5()
                    .text_ellipsis()
                    .child(format!("{}", item.title))
            })
            .collect();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(RGB_SLIGHT_100))
            .child(self.view_playlist_header.clone())
            .child(div().w_full().children(playlist_elements))
            .child(self.view_setting_header.clone())
    }
}
