use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
struct EventDispatcherInternal<Tp> {
    handler: Rc<RefCell<dyn Fn(Tp)>>,
}

#[derive(Clone)]
pub struct EventDispatcher<Tp> {
    _internal: EventDispatcherInternal<Tp>,
}

#[derive(Clone)]
pub struct EventEmitter<Tp> {
    _internal: EventDispatcherInternal<Tp>,
}

impl<Tp> EventDispatcher<Tp> {
    pub fn id(&self) -> u32 {
        todo!()
    }

    pub fn new(handler: impl Fn(Tp)) -> Self {
        todo!()
    }

    pub fn emitter(&self) -> EventEmitter<Tp> {
        todo!()
    }
}
