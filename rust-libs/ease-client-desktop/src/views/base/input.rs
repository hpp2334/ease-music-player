use std::marker::PhantomData;

use ease_client::view_models::view_state::views::playlist::VPlaylistListState;
use gpui::{div, prelude::*, px, rgb, rgba, svg, Model, SharedString, View, ViewContext};

use crate::core::{theme::{RGB_PRIMARY, RGB_PRIMARY_700, RGB_PRIMARY_TEXT, RGB_SLIGHT_300, RGB_SLIGHT_700, RGB_SURFACE}, view_state::ViewStates};

pub struct InputComponent<F>
where F: Fn(&mut gpui::WindowContext<'_>) {
    id: SharedString,
    _m: PhantomData<F>
}

impl<F> InputComponent<F>
where F: Fn(&mut gpui::WindowContext<'_>) {
    pub fn new(id: SharedString) -> Self {
        Self {
            id,
            _m: Default::default()
        }
    }
}

impl<F> Render for InputComponent<F>
where
F: Fn(&mut gpui::WindowContext<'_>) + 'static {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        
        div()
    }
}
