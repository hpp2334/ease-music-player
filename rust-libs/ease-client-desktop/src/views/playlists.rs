use ease_client::{
    view_models::view_state::views::playlist::VPlaylistListState, PlaylistListWidget, WidgetAction,
    WidgetActionType,
};
use gpui::{div, img, prelude::*, px, rgb, svg, Entity, ImageSource, SharedString};

use crate::core::{
    theme::{RGB_PRIMARY_TEXT, RGB_SLIGHT_100, RGB_SLIGHT_300},
    view_state::ViewStates,
    vm::AppBridge,
};

pub struct PlaylistListComponent {
    playlist_list: Entity<VPlaylistListState>,
}

impl PlaylistListComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        cx.focus_handle();

        let playlist_list = vs.playlist_list.clone();
        cx.observe(&playlist_list, |_, _, _| {}).detach();
        Self {
            playlist_list: vs.playlist_list.clone(),
        }
    }
}

impl Render for PlaylistListComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.playlist_list.read(cx);

        let playlist_elements: Vec<_> = state
            .playlist_list
            .clone()
            .into_iter()
            .map(|item| {
                div()
                    .id(*item.id.as_ref() as usize)
                    .cursor_pointer()
                    .w(px(150.0))
                    .h(px(200.0))
                    .p(px(4.0))
                    .text_ellipsis()
                    .hover(|style| style.bg(rgb(RGB_SLIGHT_300)))
                    .on_click({
                        let item = item.clone();
                        move |_event, _, cx| {
                            println!("VPlaylistAbstractItem {:?}", item);
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                img(ImageSource::Resource(gpui::Resource::Embedded(
                                    "drawables://CoverDefault.webp".into(),
                                )))
                                .w(px(142.0))
                                .h(px(142.0)),
                            )
                            .child(format!("{}", item.title))
                            .child(format!("{} Musics · {}", item.count, item.duration)),
                    )
            })
            .collect();

        div()
            .size_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .p(px(32.0))
            .children(playlist_elements)
            .child(
                div()
                    .id(SharedString::new_static("main-add-playlist"))
                    .w(px(150.0))
                    .h(px(200.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(RGB_SLIGHT_100))
                    .hover(|style| style.bg(rgb(RGB_SLIGHT_300)))
                    .cursor_pointer()
                    .rounded(px(20.0))
                    .on_click({
                        move |_event, _w, cx| {
                            let app = cx.global::<AppBridge>().clone();
                            app.dispatch_widget(
                                cx,
                                WidgetAction {
                                    widget: PlaylistListWidget::Add.into(),
                                    typ: WidgetActionType::Click,
                                },
                            );
                        }
                    })
                    .child(
                        svg()
                            .size(px(16.0))
                            .text_color(rgb(RGB_PRIMARY_TEXT))
                            .path("drawables://Plus.svg"),
                    ),
            )
    }
}
