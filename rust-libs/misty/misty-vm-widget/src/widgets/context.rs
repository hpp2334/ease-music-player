use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::utils::{id_alloc::IdAlloc, lazy::Lazy, rc_owned::RcOwned};

use super::{
    AnyWidget, AnyWidgets, EventDispatcher, EventDispatchers, IntoAnyWidget, ObjectAction, RenderObject, WidgetNode, WidgetTree, WithRenderObject
};

#[derive(Clone)]
pub(crate) struct WidgetVMStore {
    pub render_tree: WidgetTree,
    pub event_dispatchers: Rc<RefCell<EventDispatchers>>,
}

pub struct WidgetContext {
    pub(crate) store: WidgetVMStore,
    pub(crate) parent_atom_id: u64,
    pub(crate) widget_id: u64,
    pub(crate) to_notify_objects: Vec<ObjectAction>,
    pub(crate) dirty_widget_ids: RcOwned<Vec<u64>>,
}

impl WidgetVMStore {
    pub fn new() -> Self {
        Self {
            render_tree: WidgetTree::new(),
            event_dispatchers: Rc::new(RefCell::new(EventDispatchers::new())),
        }
    }
}

impl WidgetContext {
    pub(crate) fn new(store: WidgetVMStore) -> Self {
        Self {
            store,
            parent_atom_id: 0,
            widget_id: Default::default(),
            to_notify_objects: Default::default(),
            dirty_widget_ids: RcOwned::new(Default::default()),
        }
    }

    pub fn mark_dirty(&self) {
        self.dirty_widget_ids.get_mut().push(self.widget_id);
    }

    pub(crate) fn set_current_node(&mut self, node: &WidgetNode) {
        self.widget_id = node.id();
        self.parent_atom_id = node.parent_atom_id();
    }

    pub fn event_dispatcher<Tp>(
        &self,
        handler: impl Fn(&WidgetContext, Tp) + 'static,
    ) -> EventDispatcher<Tp> {
        let node = self.current_widget_node();
        node.event_dispatcher(self, handler)
    }

    pub fn render_object(&self, o: impl RenderObject, children: AnyWidgets) -> WithRenderObject {
        WithRenderObject::new(o, children)
    }

    fn current_widget_node(&self) -> WidgetNode {
        self.store.render_tree.get(self.widget_id)
            .expect(format!("current widget {} is invalid", self.widget_id).as_str())
    }
}
