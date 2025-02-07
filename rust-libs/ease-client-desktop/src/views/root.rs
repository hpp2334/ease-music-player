use gpui::{div, prelude::*, px, rgb, rgba, BoxShadow, Entity, Point, SharedString};

use crate::core::{
    theme::{RGB_PRIMARY_TEXT, RGB_SURFACE},
    view_state::ViewStates,
};

use super::{
    routes::RoutesComponent, sidebar::SidebarComponent,
    storage_upsert::StorageUpsertModalComponent, windowbar::WindowBarComponent,
};

pub struct RootComponent {
    routes: Entity<RoutesComponent>,
    window_bar: Entity<WindowBarComponent>,
    view_sidebar: Entity<SidebarComponent>,
    view_modal_storage_upsert: Entity<StorageUpsertModalComponent>,
}

impl RootComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        let view_modal_storage_upsert = cx.new(|cx| StorageUpsertModalComponent::new(cx, vs));

        Self {
            window_bar: cx.new(|cx| WindowBarComponent {}),
            routes: cx.new(|cx| RoutesComponent::new(cx, vs)),
            view_sidebar: cx.new(|cx| SidebarComponent::new(cx, vs)),
            view_modal_storage_upsert,
        }
    }
}

impl Render for RootComponent {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().child(
            div()
                .relative()
                .font_family(SharedString::new_static("NotoSans"))
                .text_color(rgb(RGB_PRIMARY_TEXT))
                .bg(rgb(RGB_SURFACE))
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
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .right_0()
                        .h(px(48.0))
                        .child(self.window_bar.clone()),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(48.0))
                        .bottom_0()
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top_0()
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
                        .child(self.view_modal_storage_upsert.clone()),
                ),
        )
    }
}
