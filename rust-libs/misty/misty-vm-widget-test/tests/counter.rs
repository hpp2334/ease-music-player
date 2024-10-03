use std::{borrow::BorrowMut, convert::Infallible};

use misty_vm_widget::{
    define_widget, AnyWidget, AnyWidgets, App, AppBuilderContext, EventDispatcher, IWidgetToHost,
    IntoAnyWidget, Model, RenderObject, ViewModelContext, Widget, WidgetAppBuilderExt,
    WidgetAppExt, WidgetContext, WidgetEvent, WidgetMeta, WidgetPropsState, WidgetState,
    WidgetToHost, WidgetVMEvent, WidgetViewModel,
};
use misty_vm_widget_test::{Event, HostRenderElement};

#[derive(Default, Clone)]
struct ColumnProps {
    children: AnyWidgets,
}
define_widget!(Column, ColumnProps);

impl Column {
    pub fn new() -> Self {
        Self::build(Default::default())
    }
    pub fn with_child(self, el: impl IntoAnyWidget) -> Self
    where
        Self: Sized,
    {
        self.props().children.push(el);
        self
    }
}
impl Widget for Column {
    fn render(&self, cx: &WidgetContext) -> impl IntoAnyWidget {
        cx.render_object(HostRenderElement::Column, self.props().children.clone())
    }
}

#[derive(Clone)]
struct TextProps {
    text: String,
}
define_widget!(Text, TextProps);

impl Text {
    pub fn new(text: String) -> Self {
        Self::build(TextProps { text })
    }
}
impl Widget for Text {
    fn render(&self, cx: &WidgetContext) -> impl IntoAnyWidget {
        cx.render_object(
            HostRenderElement::Text {
                text: self.props().text.clone(),
                tag: self.props().text.clone(),
            },
            AnyWidgets::new(),
        )
    }
}

#[derive(Clone)]
struct ButtonProps {
    text: String,
    on_click: EventDispatcher<()>,
}
define_widget!(Button, ButtonProps);

impl Widget for Button {
    fn render(&self, cx: &WidgetContext) -> impl IntoAnyWidget {
        cx.render_object(
            HostRenderElement::Button {
                text: self.props().text.clone(),
                tag: self.props().text.clone(),
                ed_click_id: self.props().on_click.id(),
            },
            AnyWidgets::new(),
        )
    }
}

define_widget!(Root, (), u32, Default::default);
impl Root {
    pub fn new() -> Self {
        Self::build(())
    }
}
impl Widget for Root {
    fn render(&self, cx: &WidgetContext) -> impl IntoAnyWidget {
        let state = self.state();
        let counter = *state.get(cx);
        let on_incr = {
            let state = state.clone();
            cx.event_dispatcher(move |cx, _: ()| {
                let mut state = state.get_mut(cx);
                *state += 1;
                cx.mark_dirty();
            })
        };
        let on_decr = {
            let state = state.clone();
            cx.event_dispatcher(move |cx, _: ()| {
                let mut state = state.get_mut(cx);
                *state -= 1;
                cx.mark_dirty();
            })
        };

        Column::new()
            .with_child(Text::new(format!("Counter is {counter}")))
            .with_child(Button::build(ButtonProps {
                text: "+".to_string(),
                on_click: on_incr,
            }))
            .with_child(Button::build(ButtonProps {
                text: "-".to_string(),
                on_click: on_decr,
            }))
    }
}

fn build_app(render_tree: impl IWidgetToHost) -> App {
    let app = App::builder::<Event, Infallible>()
        .with_widget(
            WidgetViewModel::new(|e| match e {
                Event::Widget(e) => Some(e),
            }),
            WidgetToHost::new(render_tree),
        )
        .build();
    app.start();
    app.render(|e| Event::Widget(e), || Root::new());
    app
}

#[cfg(test)]
mod tests {
    use misty_vm_widget_test::{assert_tree, RenderTree};

    use crate::build_app;

    #[test]
    fn test_init() {
        let tree = RenderTree::new();
        let app = build_app(tree.clone());

        assert_tree(
            &tree,
            r#"
<Container>
-- <Column>
---- <Text> [Counter is 0] Counter is 0
---- <Button> [+] +
---- <Button> [-] -
        "#,
        );
    }

    #[test]
    fn test_incr() {
        let tree = RenderTree::new();
        let app = build_app(tree.clone());

        let n = tree.find_by_tag("+").unwrap();
        n.emit_click(&app);

        assert_tree(
            &tree,
            r#"
<Container>
-- <Column>
---- <Text> [Counter is 1] Counter is 1
---- <Button> [+] +
---- <Button> [-] -
        "#,
        );
    }
}
