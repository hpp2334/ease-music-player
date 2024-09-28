use std::{borrow::BorrowMut, convert::Infallible};

use misty_vm_widget::{
    empty_widgets, AnyWidget, App, AppBuilderContext, AsWidget, EmptyWidgets, EventDispatcher,
    IntoWidget, Model, RenderObject, ViewModelContext, Widget, WidgetAtom, WidgetContext,
    WidgetEvent, WidgetRender, WidgetState, WidgetViewModel,
};

enum HostRenderElement {
    Button { text: String, ed_click_id: u32 },
    Column,
    Text { text: String },
}

enum Event {
    Widget(WidgetEvent),
}

impl RenderObject for HostRenderElement {}

struct Counter {
    pub counter: u32,
}

#[derive(Default)]
struct ColumnProps {
    children: Vec<AnyWidget>,
}
struct Column {
    props: ColumnProps,
}
impl Widget for Column {
    type State = ();
    type Props = ColumnProps;

    fn init_state() -> Self::State {
        ()
    }
}

impl Column {
    pub fn new() -> Self {
        Self {
            props: Default::default(),
        }
    }
    pub fn with_child(mut self, el: impl IntoWidget) -> Self
    where
        Self: Sized,
    {
        self.props.children.push(el.into_any());
        self
    }
}
impl WidgetAtom for Column {
    fn render_object(&self, cx: &WidgetContext) -> impl RenderObject {
        HostRenderElement::Column
    }

    fn children(&self) -> impl Iterator<Item = impl AsWidget> {
        self.props.children.iter()
    }
}

struct TextProps {
    text: String,
    children: EmptyWidgets,
}
struct Text {
    props: TextProps,
}
impl Widget for Text {
    type State = ();
    type Props = TextProps;

    fn init_state() -> Self::State {
        ()
    }
}
impl Text {
    pub fn new(text: String) -> Self {
        Self {
            props: TextProps {
                text,
                children: EmptyWidgets::default(),
            },
        }
    }
}
impl WidgetAtom for Text {
    fn children(&self) -> impl Iterator<Item = impl AsWidget> {
        self.props.children.iter()
    }
    fn render_object(&self, cx: &WidgetContext) -> impl RenderObject {
        let el = HostRenderElement::Text {
            text: self.props.text.clone(),
        };
        el
    }
}

struct ButtonProps {
    text: String,
    on_click: EventDispatcher<()>,
    children: EmptyWidgets,
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
    fn render_object(&self, cx: &WidgetContext) -> impl RenderObject {
        HostRenderElement::Button {
            text: self.props.text.clone(),
            ed_click_id: self.props.on_click.id(),
        }
    }

    fn children(&self) -> impl Iterator<Item = impl AsWidget> {
        self.props.children.iter()
    }
}

struct Root {
    state: WidgetState<u32>,
}
impl Widget for Root {
    type State = ();
    type Props = ();

    fn init_state() -> Self::State {
        ()
    }
}
impl Root {
    pub fn new() -> Self {
        Root {
            state: WidgetState::new(0),
        }
    }
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
                children: EmptyWidgets::default(),
            }))
            .with_child(Button::new(ButtonProps {
                text: "-".to_string(),
                on_click: on_decr,
                children: EmptyWidgets::default(),
            }))
    }
}

fn build_app() -> App {
    let app = App::builder::<Event, Infallible>()
        .with_view_models(|ctx, builder| {
            builder.add(WidgetViewModel::new(
                |e| match e {
                    Event::Widget(e) => Some(e),
                },
                || Root::new(),
            ));
        })
        .build();

    app.start();
    app
}

#[cfg(test)]
mod tests {
    use crate::{build_app, Counter};

    #[test]
    fn test_render() {
        let app = build_app();
    }
}
