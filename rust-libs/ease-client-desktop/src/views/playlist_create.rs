use ease_client::{
    view_models::view_state::views::playlist::VCreatePlaylistState,
    PlaylistCreateWidget, WidgetAction, WidgetActionType,
};
use gpui::{div, prelude::*, px, Entity, SharedString};

use crate::core::{view_state::ViewStates, vm::AppBridge};

use super::base::{
    button::{button, ButtonType},
    form_input::form_widget,
    modal::modal,
    text_input::TextInputComponent,
};

pub struct PlaylistCreateModalComponent {
    state: Entity<VCreatePlaylistState>,
    visible: bool,
    view_input_name: Entity<TextInputComponent>,
}

impl PlaylistCreateModalComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        let view_input_name = cx.new(|cx| TextInputComponent::new(cx));

        cx.observe(&vs.playlist_create, move |this, _, cx| {
            let vs = this.state.read(cx).clone();
            this.visible = vs.modal_open;
            this.view_input_name.update(cx, |v, _cx| {
                v.content = vs.name.into();
            });
        })
        .detach();

        cx.observe(&view_input_name, |_this, view, cx| {
            let text = view.read(cx).content.clone();
            let app = cx.global::<AppBridge>().clone();
            app.dispatch_widget(
                cx,
                WidgetAction {
                    widget: PlaylistCreateWidget::Name.into(),
                    typ: WidgetActionType::ChangeText { text: text.into() },
                },
            );
        })
        .detach();

        Self {
            state: vs.playlist_create.clone(),
            visible: false,
            view_input_name,
        }
    }
}

impl Render for PlaylistCreateModalComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        modal().visible(self.visible).child(
            div()
                .px(px(42.0))
                .py(px(42.0))
                .w(px(750.0))
                .flex()
                .flex_col()
                .child(
                    form_widget()
                        .label("PLAYLIST NAME".into())
                        .input(self.view_input_name.clone()),
                )
                .child(div().w_full().h(px(40.0)))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(px(10.0))
                        .child(
                            button(SharedString::new_static("playlist-create-add"))
                                .typ(ButtonType::Primary)
                                .text("OK".into())
                                .on_click(|cx| {
                                    let app = cx.global::<AppBridge>().clone();
                                    app.dispatch_widget(
                                        cx,
                                        WidgetAction {
                                            widget: PlaylistCreateWidget::FinishCreate.into(),
                                            typ: WidgetActionType::Click,
                                        },
                                    );
                                }),
                        )
                        .child(
                            button(SharedString::new_static("playlist-create-cancel"))
                                .text("Cancel".into())
                                .on_click(|cx| {
                                    let app = cx.global::<AppBridge>().clone();
                                    app.dispatch_widget(
                                        cx,
                                        WidgetAction {
                                            widget: PlaylistCreateWidget::Cancel.into(),
                                            typ: WidgetActionType::Click,
                                        },
                                    );
                                }),
                        ),
                ),
        )
    }
}
