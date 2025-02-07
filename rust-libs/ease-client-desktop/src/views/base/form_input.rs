use gpui::{div, prelude::*, px, Div, Entity, SharedString};

use super::input_base::BaseInputComponent;

pub struct FormInputComponent {
    label: SharedString,
    base: Option<Entity<BaseInputComponent>>,
}

impl FormInputComponent {
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

    pub fn input(mut self, view: Entity<BaseInputComponent>) -> Self {
        self.base = Some(view);
        self
    }
}

pub fn form_input() -> FormInputComponent {
    FormInputComponent::new()
}

impl IntoElement for FormInputComponent {
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
