use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

use slab::Slab;

use crate::utils::{clonable_opaque::ClonableOpaque, opaque::Opaque, rc_owned::RcOwned};

use super::{AnyWidget, WidgetContext, WidgetNodeId};


pub enum WidgetVMEvent {
    InitRender(RcOwned<Box<dyn FnOnce() -> AnyWidget>>),
    Widget(RcOwned<WidgetEvent>),
}

pub type EventDispatcherId = usize;

pub struct WidgetEvent {
    pub(crate) ed_id: EventDispatcherId,
    pub(crate) payload: Opaque,
}

#[derive(Clone)]
pub(crate) struct AnyEventDispatcher {
    node_id: WidgetNodeId,
    handler: Rc<dyn Fn(&WidgetContext, Opaque)>,
}

#[derive(Clone)]
pub struct EventDispatcher<Tp>
where
    Tp: 'static,
{
    id: EventDispatcherId,
    internal: AnyEventDispatcher,
    _marker: PhantomData<Tp>,
}

pub(crate) struct EventDispatchers {
    handlers: Slab<AnyEventDispatcher>,
}

impl AnyEventDispatcher {
    pub fn new<Tp>(node_id: WidgetNodeId, handler: impl Fn(&WidgetContext, Tp) + 'static) -> Self
    where
        Tp: 'static,
    {
        Self {
            node_id,
            handler: Rc::new(move |cx, value| handler(cx, value.downcast::<Tp>())),
        }
    }
}

impl<Tp> EventDispatcher<Tp>
where
    Tp: 'static,
{
    pub fn id(&self) -> EventDispatcherId {
        self.id
    }

    pub fn emit(&self, cx: &WidgetContext, value: Tp) {
        let value = Opaque::new(value);
        (self.internal.handler)(cx, value);
    }
}

impl EventDispatchers {
    pub fn new() -> Self {
        Self {
            handlers: Default::default(),
        }
    }

    pub fn create<Tp>(&mut self, node_id: WidgetNodeId, handler: impl Fn(&WidgetContext, Tp) + 'static) -> EventDispatcher<Tp>
    where
        Tp: 'static,
    {
        let dispatcher = AnyEventDispatcher::new(node_id, handler);
        let id = self.handlers.insert(dispatcher.clone());

        EventDispatcher {
            id,
            internal: dispatcher,
            _marker: Default::default(),
        }
    }

    fn get<Tp>(&self, id: EventDispatcherId) -> Option<EventDispatcher<Tp>> {
        let dispatcher = self.handlers.get(id);
        if let Some(dispatcher) = dispatcher {
            let dispatcher = dispatcher.clone();
            let dispatcher: EventDispatcher<Tp> = EventDispatcher {
                id,
                internal: dispatcher,
                _marker: Default::default(),
            };
            Some(dispatcher)
        } else {
            None
        }
    }

    pub fn notify(&self, cx: &mut WidgetContext, id: usize, payload: Opaque) {
        let dispatcher = self.handlers.get(id);
        if let Some(dispatcher) = dispatcher {
            let node = cx.store.render_tree.get(dispatcher.node_id);
            if let Some(node) = node {
                cx.set_current_node(&node);
                let handler = dispatcher.handler.as_ref();
                handler(cx, payload);
            }
        }
    }
}
