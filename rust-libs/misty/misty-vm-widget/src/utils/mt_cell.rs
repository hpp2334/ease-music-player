use std::{cell::{Ref, RefCell, RefMut}, ops::Deref, rc::Rc, sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard}, thread::ThreadId};

pub struct MTCell<T>
where T: 'static {
    thread_id: ThreadId,
    value: Rc<RefCell<T>>
}
unsafe impl<T> Send for MTCell<T> {}
unsafe impl<T> Sync for MTCell<T> {}

impl<T> Clone for MTCell<T> {
    fn clone(&self) -> Self {
        Self { thread_id: self.thread_id.clone(), value: self.value.clone() }
    }
}

impl<T> Default for MTCell<T>
where T: Default {
    fn default() -> Self {
        Self {
            thread_id: std::thread::current().id(),
            value: Rc::new(RefCell::new(Default::default())),
        }
    }
}


impl<T> MTCell<T> {
    pub fn new(value: T) -> Self {
        let id = std::thread::current().id();
        Self {
            thread_id: id,
            value: Rc::new(RefCell::new(value))
        }
    }

    pub fn get(&self) -> Ref<'_, T> {
        self.check_same_thread();
        self.value.borrow()
    }

    pub fn get_mut(&self) -> RefMut<'_, T> {
        self.check_same_thread();
        self.value.borrow_mut()
    }

    fn check_same_thread(&self) {
        assert!(self.thread_id == std::thread::current().id());
    }
}
