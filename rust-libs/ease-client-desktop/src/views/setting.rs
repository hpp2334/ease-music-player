use ease_client::view_models::view_state::views::storage::{VStorageListItem, VStorageListState};
use ease_client_shared::backends::storage::StorageType;
use gpui::{div, prelude::*, px, rgb, svg, Model, SharedString, ViewContext};

use crate::core::{theme::{RGB_PRIMARY_TEXT, RGB_SECONDARY_TEXT, RGB_SLIGHT_100, RGB_SLIGHT_300}, view_state::ViewStates, vm::AppBridge};

pub struct SettingComponent { 
    storage_list: Model<VStorageListState>,
}

impl SettingComponent {
    pub fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        Self {
            storage_list: vs.storage_list.clone(),
        }
    }
}

fn render_storage_block(cx: &mut ViewContext<SettingComponent>, item: VStorageListItem) -> impl IntoElement {
    let icon_path = match item.typ {
        StorageType::Local | StorageType::Webdav => "drawables://Cloud.svg",
        StorageType::OneDrive => "drawables://OneDrive.svg"
    };

    div()
        .w(px(200.0))
        .h(px(80.0))
        .bg(rgb(RGB_SLIGHT_100))
        .hover(|style| style.bg(rgb(RGB_SLIGHT_300)))
        .rounded(px(4.0))
        .px(px(18.0))
        .py(px(24.0))
        .flex()
        .flex_row()
        .child(
            svg()
                .size(px(32.0))
                .text_color(rgb(RGB_PRIMARY_TEXT))
                .path(icon_path),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(12.0))
                        .child(item.name)
                )
                .when(!item.sub_title.is_empty(), |el| {
                    el
                        .text_color(rgb(RGB_SECONDARY_TEXT))
                        .child(item.sub_title)
                })
        )
}

impl Render for SettingComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let storages = self.storage_list.read(cx).clone();
        let mut elements = vec![];
        for item in storages.items.into_iter() {
            elements.push(render_storage_block(cx, item.clone()));
        }

        div()
            .size_full()
            .p(px(48.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        svg()
                            .size(px(16.0))
                            .text_color(rgb(RGB_PRIMARY_TEXT))
                            .path("drawables://CloudStorage.svg"),
                    )
                    .child("Devices")
            )
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .children(elements)
                    .child(
                        div()
                            .id(SharedString::new_static("add-device"))
                            .w(px(80.0))
                            .h(px(80.0))
                            .bg(rgb(RGB_SLIGHT_100))
                            .hover(|style| style.bg(rgb(RGB_SLIGHT_300)))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_click(|_, cx| {
                                // cx.global::<AppBridge>().clone().dispatch(cx, action);
                            })
                            .child(
                                svg()
                                    .size(px(16.0))
                                    .text_color(rgb(RGB_PRIMARY_TEXT))
                                    .path("drawables://Plus.svg"),
                            )
                    )
            )
    }
}
