use gpui::{div, prelude::*, px, rgb, rgba, AnyElement, SharedString};

pub struct Modal {
    visible: bool,
    child: Option<AnyElement>,
}

impl Modal {
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl IntoElement for Modal {
    type Element = AnyElement;

    fn into_element(mut self) -> Self::Element {
        let c = self.child.take();

        if !self.visible {
            div().into_any_element()
        } else {
            div()
                .id(SharedString::new_static("modal-mask"))
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom_0()
                .bg(rgba(0x0000007F))
                .flex()
                .items_center()
                .justify_center()
                .on_click(|_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(rgb(0xFFFFFF))
                        .when(c.is_some(), |el| el.child(c.unwrap())),
                )
                .into_any_element()
        }
    }
}

pub fn modal() -> Modal {
    Modal {
        visible: false,
        child: None,
    }
}
