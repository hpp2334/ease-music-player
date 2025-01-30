use ease_client::view_models::view_state::views::playlist::VPlaylistListState;
use gpui::{div, prelude::*, px, rgb, rgba, svg, Model, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY, RGB_PRIMARY_700, RGB_PRIMARY_TEXT, RGB_SLIGHT_300, RGB_SLIGHT_700, RGB_SURFACE}, view_state::ViewStates};

pub enum ButtonType {
    Primary,
    Default,
}

pub struct ButtonComponent<F>
where F: Fn(&mut gpui::WindowContext<'_>) {
    id: SharedString,
    typ: ButtonType,
    on_click: Option<F>,
    text: String,
}

impl<F> ButtonComponent<F>
where F: Fn(&mut gpui::WindowContext<'_>) {
    pub fn new(id: SharedString) -> Self {
        Self {
            id,
            typ: ButtonType::Default,
            on_click: None,
            text: Default::default(),
        }
    }
    fn typ(mut self, typ: ButtonType) -> Self {
        self.typ = typ;
        self
    }
    
    fn on_click(mut self, on_click: F) -> Self {
        self.on_click = Some(on_click);
        self
    }
    
    fn text(mut self, text: String) -> Self {
        self.text = text;
        self
    }
}

impl<F> Render for ButtonComponent<F>
where
F: Fn(&mut gpui::WindowContext<'_>) + 'static {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let bg_col = match self.typ {
            ButtonType::Default => rgb(RGB_SLIGHT_300),
            ButtonType::Primary => rgb(RGB_PRIMARY),
        };
        let bg_hovered_col = match self.typ {
            ButtonType::Default => rgb(RGB_SLIGHT_700),
            ButtonType::Primary => rgb(RGB_PRIMARY_700),
        };
        
        div()
            .id(self.id.clone())
            .rounded(px(2.0))
            .px(px(12.0))
            .py(px(4.0))
            .bg(bg_col)
            .hover(|style| style.bg(bg_hovered_col))
            .on_click(|_e, cx| {

            })
            .child(self.text.clone())
    }
}
