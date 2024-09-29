use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub struct EventDispatcher<Tp> {
    id: u64,
    handler: Rc<RefCell<dyn Fn(Tp)>>,
}

impl<Tp> EventDispatcher<Tp> {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn new(id: u64, handler: impl Fn(Tp) + 'static) -> Self {
        Self {
            id,
            handler: Rc::new(RefCell::new(handler))
        }
    }
}
