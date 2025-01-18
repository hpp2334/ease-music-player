use std::{cell::RefCell, collections::HashMap};

use ease_client::view_models::view_state::views::playlist::{
    VPlaylistAbstractItem, VPlaylistListState,
};

use ease_client_shared::backends::playlist::PlaylistId;
use gpui::{
    div, prelude::*, px, rgb, rgba, size, svg, uniform_list, App, AppContext, AssetSource, Bounds,
    BoxShadow, DragMoveEvent, ElementId, Model, MouseButton, Pixels, Point, Rgba, SharedString,
    TitlebarOptions, View, ViewContext, WindowBounds, WindowDecorations, WindowOptions,
};

const RGB_PRIMARY: u32 = 0x2E89B0;
const RGB_PRIMARY_TEXT: u32 = 0x3A3A3A;
const RGB_SLIGHT: u32 = 0xF7F7F7;
const RGB_SURFACE: u32 = 0xFFFFFF;

pub struct SidebarWidget {}

impl Render for SidebarWidget {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = VPlaylistListState {
            playlist_list: vec![
                VPlaylistAbstractItem {
                    id: PlaylistId::wrap(1),
                    title: "Origami King".to_string(),
                    count: 1,
                    duration: "00:05:12".to_string(),
                    cover: None,
                },
                VPlaylistAbstractItem {
                    id: PlaylistId::wrap(2),
                    title: "サクラノ刻 -櫻の森の下を歩む- サウンドトラックCD Disc1".to_string(),
                    count: 10,
                    duration: "00:05:12".to_string(),
                    cover: None,
                },
            ],
        };

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
            .child(div().text_color(rgb(RGB_PRIMARY)).child("PLAYLISTS"))
            .child(div().w_full().h(px(300.0)).children(playlist_elements))
    }
}

struct WindowbarDrag {
    last_position: RefCell<(u32, u32)>,
}

pub struct WindowBarWidget {}

impl Render for WindowBarWidget {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h(px(48.0))
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .child(
                div()
                    .id(SharedString::new_static("window-bar-drag"))
                    .h_full()
                    .flex_grow()
                    .on_mouse_down(MouseButton::Left, |_e, cx| {
                        cx.start_window_move();
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
                            .hover(|style| style.bg(rgb(RGB_SLIGHT)))
                            .on_click(|_event, cx| {
                                cx.minimize_window();
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
                            .hover(|style| style.bg(rgb(RGB_SLIGHT)))
                            .on_click(|_event, cx| {
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

struct RootWidget {
    window_bar: View<WindowBarWidget>,
    view_sidebar: View<SidebarWidget>,
}

impl RootWidget {
    pub fn new(cx: &mut ViewContext<Self>) -> Self {
        Self {
            window_bar: cx.new_view(|cx| WindowBarWidget {}),
            view_sidebar: cx.new_view(|cx| SidebarWidget {}),
        }
    }
}
impl Render for RootWidget {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div().size_full().relative().child(
            div()
                .font_family(SharedString::new_static("NotoSans"))
                .text_color(rgb(RGB_PRIMARY_TEXT))
                .bg(rgb(RGB_SURFACE))
                .rounded_lg()
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
                        ),
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

fn patch_cwd() {
    let cwd = std::env::current_dir().unwrap();
    if cwd.ends_with("rust-libs") {
        std::env::set_current_dir(cwd.join("ease-client-desktop")).unwrap();
    }
    println!("CWD: {:?}", std::env::current_dir());
}

struct Assets {}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        const DRAWABLES_PREFIX: &'static str = "drawables://";

        std::fs::read("assets/drawables/".to_string() + &path[DRAWABLES_PREFIX.len()..])
            .map(Into::into)
            .map_err(Into::into)
            .map(Some)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        unimplemented!()
    }
}

fn main() {
    patch_cwd();

    App::new()
        .with_assets(Assets {})
        .run(|cx: &mut AppContext| {
            let bounds = Bounds::centered(None, size(px(1280.0 + 32.0), px(800.0 + 32.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    window_background: gpui::WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |cx| cx.new_view(|cx| RootWidget::new(cx)),
            )
            .unwrap();
        });
}
