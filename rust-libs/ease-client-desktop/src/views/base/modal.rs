use ease_client::view_models::view_state::views::playlist::VPlaylistListState;
use gpui::{div, prelude::*, px, rgb, rgba, svg, Model, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY_TEXT, RGB_SURFACE}, view_state::ViewStates};

pub struct ModalComponent<V> {
    visible: bool,
    child: View<V>,
}

impl<V> ModalComponent<V> {
    pub fn new(cx: &mut ViewContext<Self>, view: View<V>) -> Self {
        Self {
            visible: false,
            child: view,
        }
    }
}

impl<V> Render for ModalComponent<V>
where V: Render + 'static {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        if !self.visible {
            return div();
        }
        
        div()
            .size_full()
            .bg(rgba(0x0000002E))
            .child(
                div()
                    .bg(rgb(RGB_SURFACE))
                    .rounded(px(8.0))
                    .px(px(42.0))
                    .py(px(14.0))
                    .child(self.child.clone())
            )
            
    }
}
