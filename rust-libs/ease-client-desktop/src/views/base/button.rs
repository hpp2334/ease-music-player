use gpui::{div, prelude::*, px, rgb, Div, SharedString, Stateful};

use crate::core::theme::{
    RGB_PRIMARY, RGB_PRIMARY_700, RGB_PRIMARY_TEXT, RGB_SLIGHT_300, RGB_SLIGHT_700, RGB_SURFACE,
};

pub enum ButtonType {
    Primary,
    Default,
}

pub struct ButtonComponent {
    id: SharedString,
    typ: ButtonType,
    on_click: Option<Box<dyn Fn(&mut gpui::App)>>,
    text: String,
}

impl ButtonComponent {
    pub fn typ(mut self, typ: ButtonType) -> Self {
        self.typ = typ;
        self
    }

    pub fn on_click<F>(mut self, on_click: F) -> Self
    where
        F: Fn(&mut gpui::App) + 'static,
    {
        self.on_click = Some(Box::new(on_click));
        self
    }

    pub fn text(mut self, text: String) -> Self {
        self.text = text;
        self
    }
}

impl IntoElement for ButtonComponent {
    type Element = Stateful<Div>;

    fn into_element(mut self) -> Self::Element {
        let bg_col = match self.typ {
            ButtonType::Default => rgb(RGB_SLIGHT_300),
            ButtonType::Primary => rgb(RGB_PRIMARY),
        };
        let bg_hovered_col = match self.typ {
            ButtonType::Default => rgb(RGB_SLIGHT_700),
            ButtonType::Primary => rgb(RGB_PRIMARY_700),
        };
        let text_col = match self.typ {
            ButtonType::Default => rgb(RGB_PRIMARY_TEXT),
            ButtonType::Primary => rgb(RGB_SURFACE),
        };
        let on_click = self.on_click.take();

        div()
            .id(self.id.clone())
            .rounded(px(2.0))
            .px(px(12.0))
            .py(px(4.0))
            .bg(bg_col)
            .text_color(text_col)
            .hover(|style| style.bg(bg_hovered_col))
            .cursor_pointer()
            .on_click(move |_e, _, cx| {
                if let Some(on_click) = on_click.as_ref() {
                    on_click(cx);
                }
            })
            .child(self.text.clone())
    }
}

pub fn button(id: SharedString) -> ButtonComponent {
    ButtonComponent {
        id,
        typ: ButtonType::Default,
        on_click: None,
        text: Default::default(),
    }
}
