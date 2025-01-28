use gpui::{div, prelude::*, px, rgb, rgba, BoxShadow, Point, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY_TEXT, RGB_SURFACE}, view_state::ViewStates};

use super::{routes::RoutesComponent, sidebar::SidebarComponent, windowbar::WindowBarComponent};

pub struct RootComponent {
    routes: View<RoutesComponent>,
    window_bar: View<WindowBarComponent>,
    view_sidebar: View<SidebarComponent>,
}

impl RootComponent {
    pub fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        Self {
            window_bar: cx.new_view(|cx| WindowBarComponent {}),
            routes: cx.new_view(|cx| RoutesComponent::new(cx, vs)),
            view_sidebar: cx.new_view(|cx| SidebarComponent::new(cx, vs)),
        }
    }
}
impl Render for RootComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div().size_full().relative().child(
            div()
                .font_family(SharedString::new_static("NotoSans"))
                .text_color(rgb(RGB_PRIMARY_TEXT))
                .bg(rgb(RGB_SURFACE))
                .rounded_lg()
                .overflow_hidden()
                .left(px(16.0))
                .top(px(16.0))
                .w(px(1280.0))
                .h(px(800.0))
                .shadow(
                    vec![BoxShadow {
                        color: rgba(0x2E2E2E2E).into(),
                        offset: Point {
                            x: px(0.0),
                            y: px(0.0),
                        },
                        blur_radius: px(4.0),
                        spread_radius: px(0.0),
                    }]
                    .into(),
                )
                .relative()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(48.0))
                        .bottom_0()
                        .flex()
                        .flex_row()
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(px(200.0))
                                .h_full()
                                .child(self.view_sidebar.clone()),
                        )
                        .child(div().size_full().child(self.routes.clone())),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .right_0()
                        .h(px(48.0))
                        .child(self.window_bar.clone()),
                ),
        )
    }
}
