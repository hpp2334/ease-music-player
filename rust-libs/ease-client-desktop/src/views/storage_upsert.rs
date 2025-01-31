use ease_client::{view_models::view_state::views::storage::{VEditStorageState, VStorageListItem, VStorageListState}, StorageUpsertWidget, WidgetAction, WidgetActionType};
use ease_client_shared::backends::storage::StorageType;
use gpui::{div, prelude::*, px, rgb, svg, Model, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY, RGB_PRIMARY_700, RGB_PRIMARY_TEXT, RGB_SLIGHT_100, RGB_SLIGHT_300, RGB_SURFACE}, view_state::ViewStates, vm::AppBridge};

use super::base::{button::{button, ButtonType}, form_input::{form_input, FormInputComponent}, input_base::BaseInputComponent, modal::modal };

struct StorageBlockComponent {
    icon_path: &'static str,
    text: &'static str,
    storage_typ: StorageType,
    current_active: StorageType,
}


pub struct StorageUpsertModalComponent { 
    state: Model<VEditStorageState>,
    visible: bool,
    view_webdav_block: View<StorageBlockComponent>,
    view_onedrive_block: View<StorageBlockComponent>,
    view_input_alias: View<FormInputComponent>,
    view_input_address: View<FormInputComponent>,
    view_input_username: View<FormInputComponent>,
    view_input_password: View<FormInputComponent>,
}

impl Render for StorageBlockComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let active = self.current_active == self.storage_typ;
        let col = rgb(if active { RGB_SURFACE } else { RGB_PRIMARY_TEXT });
        let bg_col = rgb(if active { RGB_PRIMARY } else { RGB_SLIGHT_100 });
        let hovered_bg_col = rgb(if active { RGB_PRIMARY_700 } else { RGB_SLIGHT_300 });

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
            .child(
                svg()
                    .size(px(32.0))
                    .text_color(col)
                    .path(self.icon_path)
            )
            .child(self.text)
    }
}

impl StorageUpsertModalComponent {
    pub fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        let current_storage_type = vs.storage_upsert.read(cx).info.typ;
        
        cx.observe(&vs.storage_upsert, move |this, _, cx| {
            let vs = this.state.read(cx).clone();
            let current_storage_type = vs.info.typ;
            this.view_onedrive_block.update(cx, |v, cx| {
                v.current_active = current_storage_type;
            });
            this.view_webdav_block.update(cx, |v, cx| {
                v.current_active = current_storage_type;
            });
            this.visible = vs.open;
        }).detach();

        Self {
            state: vs.storage_upsert.clone(),
            visible: false,
            view_webdav_block: cx.new_view(|cx| {
                StorageBlockComponent {
                    icon_path: "drawables://Cloud.svg",
                    text: "WebDAV",
                    storage_typ: StorageType::Webdav,
                    current_active: current_storage_type,
                }
            }),
            view_onedrive_block: cx.new_view(|cx| {
                StorageBlockComponent {
                    icon_path: "drawables://OneDrive.svg",
                    text: "OneDrive",
                    storage_typ: StorageType::OneDrive,
                    current_active: current_storage_type,
                }
            }),
            view_input_alias: cx.new_view(|cx| {
                BaseInputComponent::new(cx).on_change(|w| {
                    
                });
            }),
            view_input_address: cx.new_view(|cx| {
                let base = BaseInputComponent::new(cx);
                FormInputComponent::new(cx, base).label("ADDRESS".into())
            }),
            view_input_username: cx.new_view(|cx| {
                let base = BaseInputComponent::new(cx);
                FormInputComponent::new(cx, base).label("USERNAME".into())
            }),
            view_input_password: cx.new_view(|cx| {
                let base = BaseInputComponent::new(cx);
                FormInputComponent::new(cx, base).label("PASSWORD".into())
            }),
        }
    }
}

impl Render for StorageUpsertModalComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {

        modal()
            .visible(self.visible)
            .child(
                div()
                    .px(px(42.0))
                    .py(px(42.0))
                    .w(px(750.0))
                    .h(px(442.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(16.0))
                            .child(self.view_webdav_block.clone())
                            .child(self.view_onedrive_block.clone())
                    )
                    .child(div().w_full().h(px(40.0)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .child(
                                form_input()
                                    .label("ALIAS".into())
                                    .input(self.view_input_alias.clone())
                            )
                            .child(div().w(px(32.0)).h_full())
                            .child(self.view_input_address.clone())
                    )
                    .child(div().w_full().h(px(40.0)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .child(self.view_input_username.clone())
                            .child(div().w(px(32.0)).h_full())
                            .child(self.view_input_password.clone())
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
                                button(SharedString::new_static("storage-upsert-add"))
                                    .typ(ButtonType::Primary)
                                    .text("OK".into())
                                    .on_click(|cx| {})
                            )
                            .child(
                                button(SharedString::new_static("storage-upsert-test"))
                                    .text("Test Connection".into())
                                    .on_click(|cx| {})
                            )
                            .child(
                                button(SharedString::new_static("storage-upsert-cancel"))
                                    .text("Cancel".into())
                                    .on_click(|cx| {
                                        let app = cx.global::<AppBridge>().clone();
                                        app.dispatch_widget(cx, WidgetAction {
                                            widget: StorageUpsertWidget::Cancel.into(),
                                            typ: WidgetActionType::Click,
                                        });
                                    })
                            )
                    )
            )
    }
}

