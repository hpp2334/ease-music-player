use ease_client::{
    view_models::{
        main::desktop_sidebar::DesktopSidebarWidget,
        view_state::views::playlist::VPlaylistListState,
    },
    DesktopRoutesKey, WidgetAction, WidgetActionType,
};
use gpui::{div, prelude::*, px, rgb, svg, App, Entity, SharedString};

use crate::core::{
    theme::{RGB_PRIMARY, RGB_PRIMARY_TEXT, RGB_SECONDARY_TEXT, RGB_SLIGHT_100},
    view_state::{RouteStack, ViewStates},
    vm::AppBridge,
};

fn route_id(key: DesktopRoutesKey) -> &'static str {
    match key {
        DesktopRoutesKey::Home => "route-home",
        DesktopRoutesKey::Setting => "route-setting",
    }
}

struct SiderbarHeaderComponentProps {
    icon_path: &'static str,
    text: &'static str,
    route: DesktopRoutesKey,
    widget: DesktopSidebarWidget,
}

pub struct SiderbarHeaderComponent {
    route_stack: Entity<RouteStack>,
    props: SiderbarHeaderComponentProps,
}

impl SiderbarHeaderComponent {
    fn new(
        cx: &mut App,
        route_stack: Entity<RouteStack>,
        props: SiderbarHeaderComponentProps,
    ) -> Self {
        cx.observe(&route_stack, |_, _| {}).detach();

        Self { route_stack, props }
    }
}

impl Render for SiderbarHeaderComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.route_stack.read(cx).current() == self.props.route;
        let w = self.props.widget.clone();

        div()
            .id(SharedString::new_static(route_id(self.props.route.clone())))
            .w_full()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .cursor_pointer()
            .on_click(move |_, _win, cx| {
                if !active {
                    let app = cx.global::<AppBridge>().clone();
                    app.dispatch_widget(
                        cx,
                        WidgetAction {
                            widget: w.clone().into(),
                            typ: WidgetActionType::Click,
                        },
                    );
                }
            })
            .child(div().w(px(20.0)).h_full())
            .child(
                svg()
                    .size(px(16.0))
                    .text_color(rgb(RGB_PRIMARY_TEXT))
                    .path(self.props.icon_path),
            )
            .child(div().w(px(6.0)).h_full())
            .child(
                div()
                    .flex_grow()
                    .text_color(if active {
                        rgb(RGB_PRIMARY_TEXT)
                    } else {
                        rgb(RGB_SECONDARY_TEXT)
                    })
                    .child(self.props.text),
            )
            .child(div().w(px(4.0)).h_full())
            .child(div().w(px(2.0)).h(px(16.0)).when(active, |el| {
                el.child(div().size_full().rounded_xl().bg(rgb(RGB_PRIMARY)))
            }))
    }
}
pub struct SidebarComponent {
    playlist_list: Entity<VPlaylistListState>,
    route_stack: Entity<RouteStack>,
    view_playlist_header: Entity<SiderbarHeaderComponent>,
    view_setting_header: Entity<SiderbarHeaderComponent>,
}

impl SidebarComponent {
    pub fn new(cx: &mut App, vs: &ViewStates) -> Self {
        let view_playlist_header = cx.new(|cx| {
            SiderbarHeaderComponent::new(
                cx,
                vs.route_stack.clone(),
                SiderbarHeaderComponentProps {
                    icon_path: "drawables://AlbumOutline.svg",
                    text: "Playlists",
                    route: DesktopRoutesKey::Home,
                    widget: DesktopSidebarWidget::Playlists,
                },
            )
        });
        let view_setting_header = cx.new(|cx| {
            SiderbarHeaderComponent::new(
                cx,
                vs.route_stack.clone(),
                SiderbarHeaderComponentProps {
                    icon_path: "drawables://SettingOutline.svg",
                    text: "Setting",
                    route: DesktopRoutesKey::Setting,
                    widget: DesktopSidebarWidget::Settings,
                },
            )
        });

        cx.observe(&vs.playlist_list, |_, _| {}).detach();

        Self {
            playlist_list: vs.playlist_list.clone(),
            route_stack: vs.route_stack.clone(),
            view_playlist_header: view_playlist_header.clone(),
            view_setting_header: view_setting_header.clone(),
        }
    }
}

impl Render for SidebarComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                        move |_event, _win, _cx| {
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
