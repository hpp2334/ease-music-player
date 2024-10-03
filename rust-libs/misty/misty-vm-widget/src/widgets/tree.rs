use std::{
    borrow::BorrowMut,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
};

use crate::utils::id_alloc::IdAlloc;

use super::{
    AnyWidget, EventDispatcher, EventDispatcherId, IntoAnyWidget, ObjectAction,
    RenderObjectPlaceholder, WidgetContext, WidgetRootPlaceholder,
};

pub type WidgetNodeId = u64;

struct WidgetNodeData {
    id: WidgetNodeId,
    parent_atom_id: WidgetNodeId,
    widget: AnyWidget,
    children: Vec<WidgetNode>,
    event_dispatcher_ids: Vec<EventDispatcherId>,
}

#[derive(Clone)]
pub(crate) struct WidgetNode(Rc<RefCell<WidgetNodeData>>);

#[derive(Clone)]
pub(crate) struct WidgetTree {
    root: WidgetNode,
    nodes: Rc<RefCell<HashMap<u64, WidgetNode>>>,
    id_alloc: IdAlloc,
}

impl WidgetNodeData {
    fn add_event_dispatcher_id(&mut self, id: EventDispatcherId) {
        self.event_dispatcher_ids.push(id);
    }
    fn remove_event_dispatchers(&mut self) {
        self.event_dispatcher_ids.clear();
    }
}

impl WidgetNode {
    fn new(data: WidgetNodeData) -> Self {
        Self(Rc::new(RefCell::new(data)))
    }
    fn get_ref(&self) -> Ref<'_, WidgetNodeData> {
        self.0.borrow()
    }
    fn get_mut(&self) -> RefMut<'_, WidgetNodeData> {
        RefCell::borrow_mut(&self.0)
    }

    pub fn id(&self) -> WidgetNodeId {
        self.get_ref().id
    }
    pub fn parent_atom_id(&self) -> WidgetNodeId {
        self.get_ref().parent_atom_id
    }
    pub fn event_dispatcher<Tp>(
        &self,
        cx: &WidgetContext,
        handler: impl Fn(&WidgetContext, Tp) + 'static,
    ) -> EventDispatcher<Tp> {
        let mut node = self.get_mut();
        let mut dispatchers = RefCell::borrow_mut(&cx.store.event_dispatchers);
        let dispatcher = dispatchers.create(node.id, handler);
        node.add_event_dispatcher_id(dispatcher.id());
        dispatcher
    }
}

impl WidgetTree {
    pub fn new() -> Self {
        let id_alloc = IdAlloc::new();
        let id = id_alloc.allocate();
        let root = WidgetNode::new(WidgetNodeData {
            id,
            parent_atom_id: 0,
            widget: WidgetRootPlaceholder::new_any(),
            children: Default::default(),
            event_dispatcher_ids: Default::default(),
        });

        Self {
            root,
            nodes: Default::default(),
            id_alloc,
        }
    }

    pub fn init(&self, cx: &mut WidgetContext, root_widget: Box<dyn FnOnce() -> AnyWidget>) {
        let widget = root_widget();
        self.create_tree_impl(cx, &self.root, widget);
    }

    pub fn rerender_if_dirty(&self, cx: &mut WidgetContext) {
        let ids = cx.dirty_widget_ids.take();
        if ids.is_empty() {
            return;
        }

        for id in ids {
            let node = self.get(id);
            if let Some(node) = node {
                cx.set_current_node(&node);
                self.rerender_tree(cx, node);
            }
        }
    }

    fn rerender_tree(&self, cx: &mut WidgetContext, node: WidgetNode) {
        self.remove_subtree(cx, node.clone());
        {
            let mut node = node.get_mut();
            node.remove_event_dispatchers();
        }
        self.create_subtree_impl(cx, node);
    }

    fn remove_subtree(&self, cx: &mut WidgetContext, node: WidgetNode) {
        for child in node.get_ref().children.iter() {
            self.remove_tree_impl(cx, child.clone());
        }
    }

    fn remove_tree_impl(&self, cx: &mut WidgetContext, node: WidgetNode) {
        for child in node.get_ref().children.iter() {
            self.remove_tree_impl(cx, child.clone());
        }
        let mut nodes = RefCell::borrow_mut(&self.nodes);
        let node = node.get_ref();
        if node.widget.is_atom() {
            cx.to_notify_objects.push(ObjectAction::Remove {
                id: node.id,
                parent_id: node.parent_atom_id,
            });
        }
        nodes.remove(&node.id);
    }

    pub fn get(&self, id: u64) -> Option<WidgetNode> {
        let node = self.nodes.borrow();
        let node = node.get(&id);
        if let Some(node) = node {
            Some(node.clone())
        } else {
            None
        }
    }

    fn create_tree_impl(&self, cx: &mut WidgetContext, parent: &WidgetNode, widget: AnyWidget) {
        let node = self.create_widget_node(cx, widget.clone_widget());
        parent.get_mut().children.push(node.clone());
        self.create_subtree_impl(cx, node);
    }

    fn create_subtree_impl(&self, cx: &mut WidgetContext, node: WidgetNode) {
        let id = node.0.borrow().id;
        let widget = node.0.borrow().widget.clone_widget();
        cx.set_current_node(&node);
        if widget.is_atom() {
            let (object, children) = widget.render_atom(cx);
            let node = self.create_widget_node(cx, RenderObjectPlaceholder::new_any());

            cx.to_notify_objects.push(ObjectAction::Add {
                parent_id: cx.parent_atom_id,
                id,
                data: object,
            });

            let old_parent_atom_id = cx.parent_atom_id;
            cx.parent_atom_id = id;
            for child in children.widgets().into_iter() {
                self.create_tree_impl(cx, &node, child.clone_widget());
            }
            cx.parent_atom_id = old_parent_atom_id;
        } else {
            let widget = widget.render(cx);
            self.create_tree_impl(cx, &node, widget);
        }
    }

    fn create_widget_node(&self, cx: &WidgetContext, widget: AnyWidget) -> WidgetNode {
        let id = self.id_alloc.allocate();
        let node = WidgetNode::new(WidgetNodeData {
            id,
            parent_atom_id: cx.parent_atom_id,
            widget,
            children: Default::default(),
            event_dispatcher_ids: Default::default(),
        });
        RefCell::borrow_mut(&self.nodes).insert(id, node.clone());
        node
    }
}
