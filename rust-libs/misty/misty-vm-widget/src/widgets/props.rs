use std::any::Any;

use crate::utils::clonable_opaque::ClonableOpaque;


#[derive(Clone)]
pub(crate) struct AnyProps {
    value: ClonableOpaque,
}

impl AnyProps {
    pub fn new<T>(value: T) -> Self
    where T: Clone + Any + 'static {
        Self {
            value: ClonableOpaque::new(value)
        }
    }

    pub fn downcast_ref<T>(&self) -> &T
    where T: Clone + Any + 'static {
        let value = self.value.downcast_ref::<T>();
        value
    }
}