use std::cell::{Ref, RefCell};

pub struct Lazy<T> {
    f: RefCell<Option<Box<dyn FnOnce() -> T>>>,
    value: RefCell<Option<T>>,
}

impl<T> Lazy<T> {
    pub fn new(f: impl FnOnce() -> T + 'static) -> Self {
        Self {
            f: RefCell::new(Some(Box::new(f))),
            value: RefCell::new(None),
        }
    }

    pub fn get(&self) -> Ref<'_, T> {
        let uninit = self.value.borrow().is_none();
        if uninit {
            let f = self.f.borrow_mut().take().expect("init function is None");
            let value = f();
            *self.value.borrow_mut() = Some(value);
        }
        let v = Ref::map(self.value.borrow(), |v| v.as_ref().expect("value is None"));
        v
    }
}
