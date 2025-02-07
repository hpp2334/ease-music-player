use gpui::{div, prelude::*, px, rgb, svg, MouseButton, SharedString};

use crate::core::theme::{RGB_PRIMARY_TEXT, RGB_SLIGHT_100, RGB_WINDOW_BAR};

pub struct WindowBarComponent {}

impl Render for WindowBarComponent {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .w_full()
            .h(px(48.0))
            .bg(rgb(RGB_WINDOW_BAR))
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .child(
                div()
                    .id(SharedString::new_static("window-bar-drag"))
                    .h_full()
                    .flex_grow()
                    .on_mouse_down(MouseButton::Left, |_e, win, _cx| {
                        win.start_window_move();
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(16.0))
                    .px(px(16.0))
                    .child(
                        div()
                            .id(SharedString::new_static("window-bar-minimize"))
                            .size(px(16.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(RGB_SLIGHT_100)))
                            .on_click(|_event, win, _cx| {
                                win.minimize_window();
                            })
                            .child(
                                svg()
                                    .size(px(16.0))
                                    .text_color(rgb(RGB_PRIMARY_TEXT))
                                    .path("drawables://Minimize.svg"),
                            ),
                    )
                    .child(
                        div()
                            .id(SharedString::new_static("window-bar-close"))
                            .size(px(16.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(RGB_SLIGHT_100)))
                            .on_click(|_event, _win, cx| {
                                cx.quit();
                            })
                            .child(
                                svg()
                                    .size(px(16.0))
                                    .text_color(rgb(RGB_PRIMARY_TEXT))
                                    .path("drawables://Close.svg"),
                            ),
                    ),
            )
    }
}
