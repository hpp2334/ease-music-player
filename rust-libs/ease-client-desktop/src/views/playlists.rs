use ease_client::view_models::view_state::views::playlist::VPlaylistListState;
use gpui::{div, prelude::*, px, rgb, svg, Entity, SharedString};

use crate::core::{theme::RGB_PRIMARY_TEXT, view_state::ViewStates};

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
                    .px_2()
                    .cursor_pointer()
                    .on_click({
                        let item = item.clone();
                        move |_event, _, cx| {
                            println!("VPlaylistAbstractItem {:?}", item);
                        }
                    })
                    .w(px(320.0))
                    .h(px(320.0))
                    .text_ellipsis()
                    .child(format!("{}", item.title))
            })
            .collect();

        div()
            .size_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .children(playlist_elements)
            .child(
                div()
                    .id(SharedString::new_static("main-add-playlist"))
                    .w(px(320.0))
                    .h(px(320.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click({
                        move |_event, _w, _cx| {
                            // let app = cx.global::<AppPodProxy>().get();
                            // app.emit(Action::View(ViewAction::Widget(WidgetAction {
                            //     widget: PlaylistListWidget::Add.into(),
                            //     typ: WidgetActionType::Click,
                            // })));
                            // app.emit(Action::View(ViewAction::Widget(WidgetAction {
                            //     widget: PlaylistCreateWidget::Name.into(),
                            //     typ: WidgetActionType::ChangeText { text: "ABC".into() },
                            // })));
                            // app.emit(Action::View(ViewAction::Widget(WidgetAction {
                            //     widget: PlaylistCreateWidget::FinishCreate.into(),
                            //     typ: WidgetActionType::Click,
                            // })));
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
