use std::{borrow::BorrowMut, convert::Infallible};

use misty_vm_widget::{
    AnyWidget, App, AppBuilderContext, EventDispatcher, IntoWidget, Model, ViewModelContext,
    Widget, WidgetAtom, WidgetContext, WidgetRender, WidgetState,
};

enum HostRenderElement {
    Button { text: String, ed_click_id: u32 },
    Column,
    Text { text: String },
}

struct Counter {
    pub counter: u32,
}

struct Column;
impl Widget for Column {
    type State = ();
    type Props = ();

    fn init_state() -> Self::State {
        ()
    }
}

impl Column {
    pub fn new() -> Self {
        todo!()
    }
    pub fn with_child(self, el: impl IntoWidget) -> Self
    where
        Self: Sized,
    {
        todo!()
    }
}
impl WidgetAtom for Column {
    fn update_render_object(&self, cx: &WidgetContext) {
        todo!()
    }

    fn children(&self) -> Vec<AnyWidget> {
        todo!()
    }
}

struct Text {
    text: String,
}
impl Widget for Text {
    type State = ();
    type Props = String;

    fn init_state() -> Self::State {
        ()
    }
}
impl Text {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}
impl WidgetAtom for Text {
    fn children(&self) -> Vec<AnyWidget> {
        vec![]
    }
    fn update_render_object(&self, cx: &WidgetContext) {
        let el = HostRenderElement::Text {
            text: self.text.clone(),
        };
    }
}

struct ButtonProps {
    text: String,
    on_click: EventDispatcher<()>,
}
struct Button {
    props: ButtonProps,
}
impl Widget for Button {
    type State = ();
    type Props = ButtonProps;

    fn init_state() -> Self::State {
        ()
    }
}
impl Button {
    pub fn new(props: ButtonProps) -> Self {
        Self { props }
    }
}
impl WidgetAtom for Button {
    fn update_render_object(&self, cx: &WidgetContext) {
        todo!()
    }

    fn children(&self) -> Vec<AnyWidget> {
        todo!()
    }
}

struct Root {
    state: WidgetState<u32>,
}
impl WidgetRender for Root {
    fn render(&self, cx: &WidgetContext) -> impl IntoWidget {
        let state = &self.state;
        let counter = *state.get(cx);
        let on_incr = cx.event_dispatcher(|_: ()| {
            let mut state = state.get_mut(cx);
            *state += 1;
        });
        let on_decr = cx.event_dispatcher(|_: ()| {
            let mut state = state.get_mut(cx);
            *state -= 1;
        });

        Column::new()
            .with_child(Text::new(format!("Counter is {counter}")))
            .with_child(Button::new(ButtonProps {
                text: "+".to_string(),
                on_click: on_incr,
            }))
            .with_child(Button::new(ButtonProps {
                text: "-".to_string(),
                on_click: on_decr,
            }))
    }
}

fn build_app() -> App {
    App::builder().build()
}

#[cfg(test)]
mod tests {
    use crate::{build_app, Counter};
}
