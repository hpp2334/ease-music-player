use ease_client::{
    view_models::view_state::views::storage::VEditStorageState, StorageUpsertWidget, WidgetAction,
    WidgetActionType,
};
use ease_client_shared::backends::storage::StorageType;
use gpui::{div, prelude::*, px, rgb, svg, Entity, SharedString};

use crate::core::{
    theme::{
        RGB_PRIMARY, RGB_PRIMARY_700, RGB_PRIMARY_TEXT, RGB_SLIGHT_100, RGB_SLIGHT_300, RGB_SURFACE,
    },
    view_state::ViewStates,
    vm::AppBridge,
};

use super::base::{
    button::{button, ButtonType},
    form_input::form_widget,
    modal::modal,
    switch_input::switch_input,
    text_input::TextInputComponent,
};

struct StorageBlockComponent {
    icon_path: &'static str,
    text: &'static str,
    storage_typ: StorageType,
    current_active: StorageType,
}

pub struct StorageUpsertModalComponent {
    state: Entity<VEditStorageState>,
    visible: bool,
    view_webdav_block: Entity<StorageBlockComponent>,
    view_onedrive_block: Entity<StorageBlockComponent>,
    view_input_alias: Entity<TextInputComponent>,
    view_input_address: Entity<TextInputComponent>,
    view_input_username: Entity<TextInputComponent>,
    view_input_password: Entity<TextInputComponent>,
}

impl Render for StorageBlockComponent {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.current_active == self.storage_typ;
        let col = rgb(if active {
            RGB_SURFACE
        } else {
            RGB_PRIMARY_TEXT
        });
        let bg_col = rgb(if active { RGB_PRIMARY } else { RGB_SLIGHT_100 });
        let hovered_bg_col = rgb(if active {
            RGB_PRIMARY_700
        } else {
            RGB_SLIGHT_300
        });

        div()
            .w(px(100.0))
            .h(px(100.0))
            .rounded(px(8.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_color(col)
            .bg(bg_col)
            .hover(|style| style.bg(hovered_bg_col))
            .cursor_pointer()
            .child(svg().size(px(32.0)).text_color(col).path(self.icon_path))
            .child(self.text)
    }
}

impl StorageUpsertModalComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        let _ = cx;
        let current_storage_type = vs.storage_upsert.read(cx).info.typ;
        let view_input_alias = cx.new(|cx| TextInputComponent::new(cx));
        let view_input_address = cx.new(|cx| TextInputComponent::new(cx));
        let view_input_username = cx.new(|cx| TextInputComponent::new(cx));
        let view_input_password = cx.new(|cx| TextInputComponent::new(cx));

        cx.observe(&vs.storage_upsert, move |this, _, cx| {
            let vs = this.state.read(cx).clone();
            let current_storage_type = vs.info.typ;
            this.visible = vs.open;
            this.view_onedrive_block.update(cx, |v, _cx| {
                v.current_active = current_storage_type;
            });
            this.view_webdav_block.update(cx, |v, _cx| {
                v.current_active = current_storage_type;
            });
            this.view_input_alias.update(cx, |v, _cx| {
                v.change_content(vs.info.alias.into());
            });
            this.view_input_address.update(cx, |v, _cx| {
                v.change_content(vs.info.addr.into());
            });
            this.view_input_username.update(cx, |v, _cx| {
                v.change_content(vs.info.username.into());
            });
            this.view_input_password.update(cx, |v, _cx| {
                v.change_content(vs.info.password.into());
            });
        })
        .detach();

        cx.observe(&view_input_alias, |_this, view, cx| {
            let text = view.read(cx).content.clone();
            let app = cx.global::<AppBridge>().clone();
            app.dispatch_widget(
                cx,
                WidgetAction {
                    widget: StorageUpsertWidget::Alias.into(),
                    typ: WidgetActionType::ChangeText { text: text.into() },
                },
            );
        })
        .detach();
        cx.observe(&view_input_address, |_this, view, cx| {
            let text = view.read(cx).content.clone();
            let app = cx.global::<AppBridge>().clone();
            app.dispatch_widget(
                cx,
                WidgetAction {
                    widget: StorageUpsertWidget::Address.into(),
                    typ: WidgetActionType::ChangeText { text: text.into() },
                },
            );
        })
        .detach();
        cx.observe(&view_input_username, |_this, view, cx| {
            let text = view.read(cx).content.clone();
            let app = cx.global::<AppBridge>().clone();
            app.dispatch_widget(
                cx,
                WidgetAction {
                    widget: StorageUpsertWidget::Username.into(),
                    typ: WidgetActionType::ChangeText { text: text.into() },
                },
            );
        })
        .detach();
        cx.observe(&view_input_password, |_this, view, cx| {
            let text = view.read(cx).content.clone();
            let app = cx.global::<AppBridge>().clone();
            app.dispatch_widget(
                cx,
                WidgetAction {
                    widget: StorageUpsertWidget::Password.into(),
                    typ: WidgetActionType::ChangeText { text: text.into() },
                },
            );
        })
        .detach();

        Self {
            state: vs.storage_upsert.clone(),
            visible: false,
            view_webdav_block: cx.new(|_cx| StorageBlockComponent {
                icon_path: "drawables://Cloud.svg",
                text: "WebDAV",
                storage_typ: StorageType::Webdav,
                current_active: current_storage_type,
            }),
            view_onedrive_block: cx.new(|_cx| StorageBlockComponent {
                icon_path: "drawables://OneDrive.svg",
                text: "OneDrive",
                storage_typ: StorageType::OneDrive,
                current_active: current_storage_type,
            }),
            view_input_alias,
            view_input_address,
            view_input_username,
            view_input_password,
        }
    }
}

impl Render for StorageUpsertModalComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_anonymous = self.state.read(cx).info.is_anonymous;

        modal().visible(self.visible).child(
            div()
                .px(px(42.0))
                .py(px(42.0))
                .w(px(750.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(16.0))
                        .child(self.view_webdav_block.clone())
                        .child(self.view_onedrive_block.clone()),
                )
                .child(div().w_full().h(px(20.0)))
                .child(
                    form_widget().label("ANONYMOUS".into()).input(
                        switch_input(SharedString::new_static("storage-upsert-anonymous").into())
                            .value(is_anonymous)
                            .on_click(|_, cx| {
                                let app = cx.global::<AppBridge>().clone();
                                app.dispatch_widget(
                                    cx,
                                    WidgetAction {
                                        widget: StorageUpsertWidget::IsAnonymous.into(),
                                        typ: WidgetActionType::Click,
                                    },
                                );
                            }),
                    ),
                )
                .child(div().w_full().h(px(20.0)))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .child(
                            form_widget()
                                .label("ALIAS".into())
                                .input(self.view_input_alias.clone()),
                        )
                        .child(div().w(px(32.0)).h_full())
                        .child(
                            form_widget()
                                .label("ADDRESS".into())
                                .input(self.view_input_address.clone()),
                        ),
                )
                .when(!is_anonymous, |el| {
                    el.child(div().w_full().h(px(20.0))).child(
                        div()
                            .flex()
                            .flex_row()
                            .child(
                                form_widget()
                                    .label("USERNAME".into())
                                    .input(self.view_input_username.clone()),
                            )
                            .child(div().w(px(32.0)).h_full())
                            .child(
                                form_widget()
                                    .label("PASSWORD".into())
                                    .input(self.view_input_password.clone()),
                            ),
                    )
                })
                .child(div().w_full().h(px(40.0)))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(px(10.0))
                        .child(
                            button(SharedString::new_static("storage-upsert-add"))
                                .typ(ButtonType::Primary)
                                .text("OK".into())
                                .on_click(|cx| {
                                    let app = cx.global::<AppBridge>().clone();
                                    app.dispatch_widget(
                                        cx,
                                        WidgetAction {
                                            widget: StorageUpsertWidget::Finish.into(),
                                            typ: WidgetActionType::Click,
                                        },
                                    );
                                }),
                        )
                        .child(
                            button(SharedString::new_static("storage-upsert-test"))
                                .text("Test Connection".into())
                                .on_click(|cx| {}),
                        )
                        .child(
                            button(SharedString::new_static("storage-upsert-cancel"))
                                .text("Cancel".into())
                                .on_click(|cx| {
                                    let app = cx.global::<AppBridge>().clone();
                                    app.dispatch_widget(
                                        cx,
                                        WidgetAction {
                                            widget: StorageUpsertWidget::Cancel.into(),
                                            typ: WidgetActionType::Click,
                                        },
                                    );
                                }),
                        ),
                ),
        )
    }
}
