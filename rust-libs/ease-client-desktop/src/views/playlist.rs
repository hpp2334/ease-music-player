use ease_client::{
    view_models::view_state::views::playlist::VCurrentPlaylistState, PlaylistDetailWidget,
    WidgetAction, WidgetActionType,
};
use gpui::{div, img, prelude::*, px, rgb, svg, Entity, ImageSource};

use crate::core::{theme::RGB_PRIMARY_TEXT, view_state::ViewStates, vm::AppBridge};

use super::base::button::button;

pub struct PlaylistComponent {
    state: Entity<VCurrentPlaylistState>,
}

impl PlaylistComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        let state = vs.playlist.clone();

        cx.observe(&state, |_, _, _| {}).detach();

        Self { state }
    }
}

impl Render for PlaylistComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx).clone();
        let music_count = state.items.len();

        let col = RGB_PRIMARY_TEXT;

        let music_elements: Vec<_> = state
            .items
            .into_iter()
            .map(|item| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .relative()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(
                                svg()
                                    .size(px(24.0))
                                    .text_color(rgb(col))
                                    .path("drawables://MusicNote.svg"),
                            )
                            .child(item.title),
                    )
                    .child(div().text_color(rgb(col)).child(item.duration))
                    .child(
                        svg()
                            .right(px(10.0))
                            .bottom(px(5.0))
                            .size(px(24.0))
                            .text_color(rgb(col))
                            .path("drawables://MusicNote.svg"),
                    )
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .w_full()
            .p(px(40.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        img(ImageSource::Resource(gpui::Resource::Embedded(
                            "drawables://CoverDefault.webp".into(),
                        )))
                        .w(px(150.0))
                        .h(px(150.0))
                        .rounded(px(20.0)),
                    )
                    .child(div().w(px(30.0)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(24.0)).child(state.title))
                            .child(
                                div().child(format!(
                                    "{} Musics  ·  {}",
                                    music_count, state.duration
                                )),
                            ),
                    ),
            )
            .child(div().h(px(30.0)))
            .child(
                div().flex().flex_row().w_full().justify_end().child(
                    button("playlist-import-music".into())
                        .text("Add Musics".into())
                        .on_click(|cx| {
                            let app = cx.global::<AppBridge>().clone();
                            app.dispatch_widget(
                                cx,
                                WidgetAction {
                                    widget: PlaylistDetailWidget::Import.into(),
                                    typ: WidgetActionType::Click,
                                },
                            );
                        }),
                ),
            )
            .child(div().flex().flex_col().children(music_elements))
    }
}
