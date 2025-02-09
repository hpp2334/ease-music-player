use ease_client::{
    view_models::view_state::views::storage::{VStorageListItem, VStorageListState},
    StorageListWidget, WidgetAction, WidgetActionType,
};
use ease_client_shared::backends::storage::StorageType;
use gpui::{div, prelude::*, px, rgb, svg, Div, ElementId, Entity, SharedString};

use crate::core::{
    theme::{RGB_PRIMARY_TEXT, RGB_SECONDARY_TEXT, RGB_SLIGHT_100, RGB_SLIGHT_300},
    view_state::ViewStates,
    vm::AppBridge,
};

pub struct SettingComponent {
    storage_list: Entity<VStorageListState>,
}

impl SettingComponent {
    pub fn new(cx: &mut Context<Self>, vs: &ViewStates) -> Self {
        Self {
            storage_list: vs.storage_list.clone(),
        }
    }
}

fn render_storage_block(item: VStorageListItem) -> impl IntoElement {
    let icon_path = match item.typ {
        StorageType::Local | StorageType::Webdav => "drawables://Cloud.svg",
        StorageType::OneDrive => "drawables://OneDrive.svg",
    };
    let id = item.storage_id;

    div()
        .id(ElementId::NamedInteger(
            "storage-block".into(),
            *item.storage_id.as_ref() as usize,
        ))
        .w(px(200.0))
        .h(px(80.0))
        .bg(rgb(RGB_SLIGHT_100))
        .hover(|style| style.bg(rgb(RGB_SLIGHT_300)))
        .rounded(px(4.0))
        .px(px(18.0))
        .py(px(24.0))
        .flex()
        .flex_row()
        .cursor_pointer()
        .gap(px(12.0))
        .on_click(move |_, _, cx| {
            let app = cx.global::<AppBridge>().clone();
            app.dispatch_widget(
                cx,
                WidgetAction {
                    widget: StorageListWidget::Item { id }.into(),
                    typ: WidgetActionType::Click,
                },
            );
        })
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
                        .text_color(rgb(RGB_PRIMARY_TEXT))
                        .text_size(px(12.0))
                        .child(item.name),
                )
                .when(!item.sub_title.is_empty(), |el| {
                    el.text_color(rgb(RGB_SECONDARY_TEXT))
                        .text_size(px(10.0))
                        .child(item.sub_title)
                }),
        )
}

impl Render for SettingComponent {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let storages = self.storage_list.read(cx).clone();
        let storage_items: Vec<_> = storages
            .items
            .into_iter()
            .filter(|v| v.typ != StorageType::Local)
            .collect();
        let mut elements = vec![];
        for item in storage_items {
            elements.push(render_storage_block(item));
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
                    .child("Devices"),
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
                            .cursor_pointer()
                            .on_click(|_, _, cx| {
                                let app = cx.global::<AppBridge>().clone();
                                app.dispatch_widget(
                                    cx,
                                    WidgetAction {
                                        widget: StorageListWidget::Create.into(),
                                        typ: WidgetActionType::Click,
                                    },
                                );
                            })
                            .child(
                                svg()
                                    .size(px(16.0))
                                    .text_color(rgb(RGB_PRIMARY_TEXT))
                                    .path("drawables://Plus.svg"),
                            ),
                    ),
            )
    }
}
