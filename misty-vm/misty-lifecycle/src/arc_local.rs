use std::{any::Any, fmt::Debug, ops::Deref, rc::Rc, thread::ThreadId};

#[derive(Debug, Clone, Copy)]
pub struct ArcLocalCore {
    id: ThreadId,
}

pub struct ArcLocal<T>
where
    T: ?Sized,
{
    core: ArcLocalCore,
    value: Rc<T>,
}

#[derive(Clone)]
pub struct ArcLocalAny {
    core: ArcLocalCore,
    value: Rc<dyn Any>,
}

impl ArcLocalCore {
    pub fn new() -> Self {
        Self {
            id: std::thread::current().id(),
        }
    }
    pub fn check_same_thread(&self) {
        assert_eq!(self.id, std::thread::current().id(), "Thread mismatch");
    }
    pub fn debug_check_same_thread(&self) {
        debug_assert_eq!(self.id, std::thread::current().id(), "Thread mismatch");
    }
}

impl<T> ArcLocal<T>
where
    T: 'static,
{
    pub fn new(core: ArcLocalCore, value: T) -> Self {
        Self {
            core,
            value: Rc::new(value),
        }
    }
    pub fn as_any(self) -> ArcLocalAny
    where
        T: Sized,
    {
        self.core.check_same_thread();
        ArcLocalAny {
            core: self.core,
            value: self.value,
        }
    }
}

impl<T> ArcLocal<T>
where
    T: ?Sized + 'static,
{
    pub fn get(&self) -> &Rc<T> {
        self.core.check_same_thread();
        &self.value
    }
}

impl<T> Clone for ArcLocal<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        self.core.check_same_thread();
        Self {
            core: self.core,
            value: self.value.clone(),
        }
    }
}

unsafe impl<T> Send for ArcLocal<T> where T: ?Sized {}
unsafe impl<T> Sync for ArcLocal<T> where T: ?Sized {}
unsafe impl Send for ArcLocalAny {}
unsafe impl Sync for ArcLocalAny {}

impl<T: Debug> Debug for ArcLocal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcLocal")
            .field("core", &self.core)
            .field("value", &self.value)
            .finish()
    }
}

impl<T> Deref for ArcLocal<T>
where
    T: ?Sized + 'static,
{
    type Target = Rc<T>;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl ArcLocalAny {
    pub fn try_downcast<T: 'static>(self) -> Option<ArcLocal<T>> {
        self.core.check_same_thread();
        self.value.downcast().ok().map(|value| ArcLocal {
            core: self.core,
            value,
        })
    }
}

impl Debug for ArcLocalAny {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcLocalAny")
            .field("core", &self.core)
            .field("value", &"<any>")
            .finish()
    }
}
