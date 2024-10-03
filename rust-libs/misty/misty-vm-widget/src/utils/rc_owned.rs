use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

#[derive(Clone)]
pub struct RcOwned<T> {
    value: Rc<RefCell<Option<T>>>,
}

impl<T> RcOwned<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Rc::new(RefCell::new(Some(value))),
        }
    }

    pub fn take(&self) -> T {
        let mut v = self.value.borrow_mut();
        v.take().unwrap()
    }

    pub fn get_mut(&self) -> RefMut<'_, T> {
        let v = self.value.borrow_mut();
        RefMut::map(v, |v| v.as_mut().unwrap())
    }
}
