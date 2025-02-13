use gpui::{
    div, prelude::FluentBuilder, px, rgb, App, Div, ElementId, InteractiveElement, IntoElement,
    ParentElement, Render, Stateful, StatefulInteractiveElement, Styled, Window,
};

use crate::core::theme::{
    RGB_PRIMARY, RGB_PRIMARY_200, RGB_PRIMARY_700, RGB_PRIMARY_TEXT, RGB_SLIGHT_100,
};

pub struct SwitchInputComponent {
    id: ElementId,
    value: bool,
    handler: Option<Box<dyn Fn(&mut Window, &mut App)>>,
}

impl SwitchInputComponent {
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.handler = Some(Box::new(f));
        self
    }
}

impl IntoElement for SwitchInputComponent {
    type Element = Stateful<Div>;

    fn into_element(mut self) -> Self::Element {
        const PAD: f32 = 7.0;
        let handler = self.handler.take();
        let bg_col = if !self.value {
            rgb(RGB_SLIGHT_100)
        } else {
            rgb(RGB_PRIMARY_200)
        };
        let handle_col = if !self.value {
            rgb(RGB_PRIMARY_TEXT)
        } else {
            rgb(RGB_PRIMARY)
        };

        div()
            .id(self.id.clone())
            .relative()
            .w(px(69.0))
            .h(px(38.0))
            .bg(bg_col)
            .rounded(px(99.0))
            .cursor_pointer()
            .when(handler.is_some(), |el| {
                el.on_click(move |_, win, app| {
                    let handler = handler.as_ref().unwrap();
                    handler(win, app);
                })
            })
            .child(
                div()
                    .absolute()
                    .w(px(24.0))
                    .h(px(24.0))
                    .top(px(PAD))
                    .bottom(px(PAD))
                    .when(!self.value, |el| el.left(px(PAD)))
                    .when(self.value, |el| el.right(px(PAD)))
                    .bg(handle_col)
                    .rounded_full(),
            )
    }
}

impl Render for SwitchInputComponent {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> impl gpui::IntoElement {
        const PAD: f32 = 7.0;
        let bg_col = if !self.value {
            rgb(RGB_SLIGHT_100)
        } else {
            rgb(RGB_PRIMARY_700)
        };
        let handle_col = if !self.value {
            rgb(RGB_PRIMARY_TEXT)
        } else {
            rgb(RGB_PRIMARY)
        };

        div()
            .id(self.id.clone())
            .relative()
            .w(px(69.0))
            .h(px(38.0))
            .bg(bg_col)
            .rounded(px(99.0))
            .on_click(cx.listener(|v, _, _, cx| {
                v.value = !v.value;
                cx.notify();
            }))
            .child(
                div()
                    .top(px(PAD))
                    .bottom(px(PAD))
                    .when(!self.value, |el| el.left(px(PAD)))
                    .when(self.value, |el| el.right(px(PAD)))
                    .bg(handle_col)
                    .rounded_full(),
            )
    }
}

pub fn switch_input(id: ElementId) -> SwitchInputComponent {
    SwitchInputComponent {
        id,
        value: false,
        handler: None,
    }
}
