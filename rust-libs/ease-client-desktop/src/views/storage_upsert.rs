use ease_client::view_models::view_state::views::storage::{VEditStorageState, VStorageListItem, VStorageListState};
use ease_client_shared::backends::storage::StorageType;
use gpui::{div, prelude::*, px, rgb, svg, Model, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY, RGB_PRIMARY_TEXT, RGB_SLIGHT_100, RGB_SLIGHT_300, RGB_SURFACE}, view_state::ViewStates};

struct StorageBlockComponent {
    icon_path: &'static str,
    text: &'static str,
    storage_typ: StorageType,
    current_active: StorageType,
}

struct StorageUpsertBodyComponent {
    state: Model<VEditStorageState>,
    view_webdav_block: View<StorageBlockComponent>,
    view_onedrive_block: View<StorageBlockComponent>,
}

pub struct StorageUpsertModalComponent { 
    state: Model<VEditStorageState>,
    child: View<StorageUpsertBodyComponent>,
}

impl Render for StorageBlockComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let active = self.current_active == self.storage_typ;
        let col = rgb(if active { RGB_SURFACE } else { RGB_PRIMARY_TEXT });
        let bg_col = rgb(if active { RGB_PRIMARY } else { RGB_SLIGHT_100 });

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
            .when(active, |el| el.bg(rgb(RGB_SLIGHT_300)))
            .child(
                svg()
                    .size(px(32.0))
                    .text_color(col)
                    .path(self.icon_path)
            )
            .child(self.text)
    }
}

impl StorageUpsertBodyComponent {
    fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        let current_storage_type = vs.storage_upsert.read(cx).info.typ;
        
        {
            let storage_upsert = vs.storage_upsert.clone();
            cx.observe(&vs.storage_upsert, move |view, _, cx| {
                let current_storage_type = storage_upsert.read(cx).info.typ;
                view.view_onedrive_block.update(cx, |v, cx| {
                    v.current_active = current_storage_type;
                });
                view.view_webdav_block.update(cx, |v, cx| {
                    v.current_active = current_storage_type;
                });
            }).detach();
        }

        Self {
            state: vs.storage_upsert.clone(),
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
        }
    }
}

impl Render for StorageUpsertBodyComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .size_full()
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
            .child(div().size_full().h(px(40.0)))
    }
}


impl StorageUpsertModalComponent {
    pub fn new(cx: &mut ViewContext<Self>, vs: &ViewStates) -> Self {
        cx.observe(&vs.storage_upsert, |_,_,_| {}).detach();

        Self {
            state: vs.storage_upsert.clone(),
            child: cx.new_view(|cx| {
                StorageUpsertBodyComponent::new(cx, vs)
            })
        }
    }
}

impl Render for StorageUpsertModalComponent {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let visible = self.state.read(cx).open;

        if !visible {
            div()
        } else {
            div()
                .size_full()
                .child(self.child.clone())
        }
    }
}
