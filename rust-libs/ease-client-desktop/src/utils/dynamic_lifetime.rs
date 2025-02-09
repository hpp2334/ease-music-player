use std::{
    cell::{RefCell, RefMut},
    ptr::null_mut,
    rc::Rc,
};

pub struct DynamicLifetime<T> {
    ptr: *mut T,
}

// HACK
pub struct SharedDynamicLifetime<T> {
    inner: Rc<RefCell<DynamicLifetime<T>>>,
}

pub struct DynamicLifeTimeGuard<T> {
    owner: SharedDynamicLifetime<T>,
}

// HACK
impl<T> SharedDynamicLifetime<T> {
    pub fn wrap(&self, _value: &mut T) -> DynamicLifeTimeGuard<T> {
        unsafe {
            self.inner.borrow_mut().set(_value);
        }

        DynamicLifeTimeGuard {
            owner: self.clone(),
        }
    }

    pub fn get(&self) -> RefMut<DynamicLifetime<T>> {
        self.inner.borrow_mut()
    }
}

impl<T> Default for SharedDynamicLifetime<T> {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(DynamicLifetime::new())),
        }
    }
}

impl<T> Clone for SharedDynamicLifetime<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> DynamicLifetime<T> {
    pub fn new() -> Self {
        Self { ptr: null_mut() }
    }

    pub fn get(&mut self) -> &mut T {
        if self.ptr.is_null() {
            panic!("ptr is null");
        }

        unsafe { self.ptr.as_mut().unwrap() }
    }

    unsafe fn set(&mut self, _value: &mut T) {
        if !self.ptr.is_null() {
            panic!("ptr is not null");
        }

        self.ptr = _value;
    }

    unsafe fn reset(&mut self) {
        if self.ptr.is_null() {
            panic!("ptr is null");
        }
        self.ptr = null_mut();
    }
}

impl<T> Default for DynamicLifetime<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for DynamicLifeTimeGuard<T> {
    fn drop(&mut self) {
        let mut owner = self.owner.inner.borrow_mut();
        unsafe {
            owner.reset();
        }
    }
}
