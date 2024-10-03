use std::{
    borrow::Borrow,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ops::Deref,
    rc::Rc,
    sync::{RwLockReadGuard, RwLockWriteGuard},
};

use misty_vm_widget::{
    utils::{mt_cell::MTCell, rc_owned::RcOwned},
    App, EventDispatcherId, IWidgetToHost, ObjectAction, RenderObject, WidgetAppExt, WidgetEvent,
    WidgetNodeId, WidgetVMEvent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRenderElement {
    Container,
    Button {
        tag: String,
        text: String,
        ed_click_id: EventDispatcherId,
    },
    Column,
    Text {
        tag: String,
        text: String,
    },
}

impl RenderObject for HostRenderElement {}

pub enum Event {
    Widget(WidgetVMEvent),
}

pub struct NodeData {
    id: u64,
    el: HostRenderElement,
    children: Vec<Node>,
}
#[derive(Clone)]
pub struct Node(MTCell<NodeData>);
#[derive(Clone)]
struct NodeMap(MTCell<HashMap<WidgetNodeId, Node>>);
struct RenderTreeData {
    map: NodeMap,
    root: Node,
}
#[derive(Clone)]
pub struct RenderTree {
    tree: MTCell<RenderTreeData>,
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
    pub fn emit_click(&self, app: &App) {
        match self.get().el.clone() {
            HostRenderElement::Button { ed_click_id, .. } => {
                app.emit_widget_event(|e| Event::Widget(e), ed_click_id, ());
            }
            _ => {
                panic!("cannot emit click event");
            }
        }
    }
    pub fn debug_fmt(&self) -> String {
        match self.get().el.clone() {
            HostRenderElement::Container => "<Container>".to_string(),
            HostRenderElement::Button { tag, text, .. } => {
                format!("<Button> [{tag}] {text}")
            }
            HostRenderElement::Column => {
                format!("<Column>")
            }
            HostRenderElement::Text { tag, text } => {
                format!("<Text> [{tag}] {text}")
            }
        }
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
    pub fn get(&self, id: WidgetNodeId) -> Node {
        let map = self.0.get();
        let n = map
            .get(&id)
            .expect(format!("fail to get node {id}").as_str());
        n.clone()
    }
    pub fn remove(&self, id: WidgetNodeId) {
        let mut map = self.0.get_mut();
        map.remove(&id);
    }
}

impl RenderTree {
    pub fn new() -> Self {
        let map = NodeMap::new();
        let root = Node::new(NodeData {
            id: 0,
            el: HostRenderElement::Container,
            children: Default::default(),
        });
        map.add(root.clone());
        Self {
            tree: MTCell::new(RenderTreeData { map, root }),
        }
    }
    pub fn root(&self) -> Node {
        let r = self.tree.get();
        let r = r.root.clone();
        r
    }

    fn debug_fmt(&self) -> String {
        let mut s: String = Default::default();
        self.debug_fmt_impl(&mut s, 0, self.root());
        s.trim().to_string()
    }

    fn debug_fmt_impl(&self, w: &mut String, dpt: usize, node: Node) {
        let prefix = "-".repeat(dpt * 2) + " ";
        *w += (prefix + node.debug_fmt().as_str() + "\n").as_str();

        for child in node.children() {
            self.debug_fmt_impl(w, dpt + 1, child);
        }
    }
    pub fn find_by_tag(&self, target: &str) -> Option<Node> {
        let target = target.to_string();
        let mut nodes = self.find_all_by(Rc::new(move |n| match n.get().el.clone() {
            HostRenderElement::Text { tag, .. } => tag == target,
            HostRenderElement::Button { tag, .. } => tag == target,
            _ => false,
        }));

        nodes.pop()
    }

    fn find_all_by(&self, f: Rc<dyn Fn(&Node) -> bool>) -> Vec<Node> {
        let mut nodes: Vec<Node> = Default::default();
        self.find_all_by_impl(self.root(), &mut nodes, f);
        nodes
    }
    fn find_all_by_impl(&self, node: Node, w: &mut Vec<Node>, f: Rc<dyn Fn(&Node) -> bool>) {
        if f(&node) {
            w.push(node.clone());
        }
        for child in node.children() {
            self.find_all_by_impl(child, w, f.clone());
        }
    }
}
impl IWidgetToHost for RenderTree {
    fn notify_render_objects(&self, actions: Vec<ObjectAction>) {
        for action in actions.into_iter() {
            match action {
                ObjectAction::Add {
                    id,
                    parent_id,
                    data,
                } => {
                    let p = self.tree.get().map.get(parent_id);
                    let n = Node::new(NodeData {
                        id,
                        el: data.downcast::<HostRenderElement>(),
                        children: Default::default(),
                    });
                    p.get_mut().children.push(n.clone());
                    self.tree.get().map.add(n);
                }
                ObjectAction::Remove { id, parent_id } => {
                    let p = self.tree.get().map.get(parent_id);
                    p.get_mut().children.retain(|v| v.get().id != id);
                    self.tree.get().map.remove(id);
                }
            }
        }
    }
}

pub fn assert_tree(tree: &RenderTree, expected: &str) {
    let snapshot = tree.debug_fmt();
    let expected = expected.trim();

    if snapshot != expected {
        println!("snapshot is\n{}", snapshot);
        println!("expection is\n{}", expected);
        assert!(false);
    } else {
        assert!(true);
    }
}
