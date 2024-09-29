use std::{borrow::BorrowMut, convert::Infallible};

use misty_vm_widget::{
    define_widget, AnyWidget, AnyWidgets, App, AppBuilderContext, EventDispatcher, IWidgetToHost, IntoAnyWidget, Model, RenderObject, ViewModelContext, Widget, WidgetContext, WidgetEvent, WidgetMeta, WidgetMetaExt, WidgetState, WidgetToHost, WidgetViewModel
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum HostRenderElement {
    Container,
    Button { text: String, ed_click_id: u64 },
    Column,
    Text { text: String },
}

enum Event {
    Widget(WidgetEvent),
}

impl RenderObject for HostRenderElement {}

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
        Self::build(TextProps {
            text,
        })
    }
}
impl Widget for Text {
    fn render(&self, cx: &WidgetContext) -> impl IntoAnyWidget {
        cx.render_object(
            HostRenderElement::Text {
                text: self.props().text.clone(),
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
        let on_incr = cx.event_dispatcher(move |_: ()| {
            // let mut state = state.get_mut(cx);
            // *state += 1;
        });
        let on_decr = cx.event_dispatcher(move |_: ()| {
            // let mut state = state.get_mut(cx);
            // *state -= 1;
        });

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
        .with_view_models(|ctx, builder| {
            builder.add(WidgetViewModel::new(
                |e| match e {
                    Event::Widget(e) => Some(e),
                },
                || Root::new(),
            ));
        })
        .with_to_hosts(|builder| {
            builder.add(WidgetToHost::new(render_tree));
        })
        .build();

    app.start();
    app
}

#[cfg(test)]
mod tests {
    use std::{borrow::Borrow, cell::{Ref, RefCell, RefMut}, collections::HashMap, ops::Deref, rc::Rc, sync::{RwLockReadGuard, RwLockWriteGuard}};

    use misty_vm_widget::{utils::mt_cell::MTCell, IWidgetToHost, ObjectAction};

    use crate::{build_app, HostRenderElement};

    struct NodeData {
        id: u64,
        el: HostRenderElement,
        children: Vec<Node>
    }
    #[derive(Clone)]
    struct Node(MTCell<NodeData>);
    #[derive(Clone)]
    struct NodeMap(MTCell<HashMap<u64, Node>>);
    struct RenderTreeData {
        map: NodeMap,
        root: Node
    }
    #[derive(Clone)]
    struct RenderTree {
        tree: MTCell<RenderTreeData>
    }

    impl Node {
        pub fn new(data: NodeData) -> Self {
            Self(MTCell::new(data))
        }
        pub fn get(&self) -> Ref<'_, NodeData> {
            self.0.get()
        }
        pub fn get_mut(&self) -> RefMut<'_, NodeData> {
            self.0.get_mut()
        }
        pub fn add_child(&self, node: Node) {
            let mut r = self.0.get_mut();
            r.children.push(node);
        }
        pub fn children(&self) -> Vec<Node> {
            self.0.get().children.clone()
        }
    }
    impl NodeMap {
        pub fn new() -> Self {
            Self(Default::default())
        }
        pub fn add(&self, node: Node) {
            let id = node.get().id;
            self.0.get_mut().insert(id, node);
        }
        pub fn get(&self, id: u64) -> Node {
            let map = self.0.get();
            let n = map.get(&id).expect(format!("fail to get node {id}").as_str());
            n.clone()
        }
    }

    impl RenderTree {
        pub fn new() -> Self {
            let map = NodeMap::new();
            let root = Node::new(NodeData { id: 0, el: HostRenderElement::Container, children: Default::default() });
            map.add(root.clone());
            Self {
                tree: MTCell::new(RenderTreeData {
                    map,
                    root,
                })
            }
        }
        pub fn root(&self) -> Node {
            let r = self.tree.get();
            let r = r.root.clone();
            r
        }
    }
    impl IWidgetToHost for RenderTree {
        fn notify_render_objects(&self, actions: Vec<ObjectAction>) {
            for action in actions.into_iter() {
                match action {
                    ObjectAction::Add { id, parent_id, data } => {
                        let p = self.tree.get().map.get(parent_id);
                        let n = Node::new(NodeData {
                            id,
                            el: data.downcast::<HostRenderElement>(),
                            children: Default::default(),
                        });
                        p.get_mut().children.push(n.clone());
                        self.tree.get().map.add(n);
                    },
                    ObjectAction::Remove { id, parent_id } => {
                        todo!()
                    }
                }
            }
        }
    }

    #[test]
    fn test_render() {
        let tree = RenderTree::new();
        let app = build_app(tree.clone());
        
        let n = tree.root();
        assert_eq!(n.get().el, HostRenderElement::Container);

        let n = n.children();
        assert_eq!(n.len(), 1);
        let n = n[0].clone();
        assert_eq!(n.get().el, HostRenderElement::Column);

        let n = n.children();
        assert_eq!(n.len(), 3);
        assert_eq!(n[0].get().el, HostRenderElement::Text { text: "Counter is 0".to_string() });
        assert_eq!(n[1].get().el, HostRenderElement::Button { text: "+".to_string(), ed_click_id: 1 });
        assert_eq!(n[2].get().el, HostRenderElement::Button { text: "-".to_string(), ed_click_id: 2 });
    }
}
