use gpui::{div, prelude::*, px, AnyElement, Div, Entity, SharedString};

use super::text_input::TextInputComponent;

pub struct FormWidgetComponent {
    label: SharedString,
    base: Option<AnyElement>,
}

impl FormWidgetComponent {
    pub fn new() -> Self {
        Self {
            label: Default::default(),
            base: None,
        }
    }

    pub fn label(mut self, text: SharedString) -> Self {
        self.label = text;
        self
    }

    pub fn input(mut self, el: impl IntoElement) -> Self {
        self.base = Some(el.into_any_element());
        self
    }
}

pub fn form_widget() -> FormWidgetComponent {
    FormWidgetComponent::new()
}

impl IntoElement for FormWidgetComponent {
    type Element = Div;

    fn into_element(mut self) -> Self::Element {
        let base = self.base.take();

        div()
            .flex()
            .flex_col()
            .w_full()
            .text_size(px(12.0))
            .child(self.label.clone())
            .when(base.is_some(), |el| el.child(base.unwrap()))
    }
}
