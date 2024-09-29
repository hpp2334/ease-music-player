use std::any::Any;

use crate::utils::clonable_opaque::ClonableOpaque;


#[derive(Clone)]
pub struct AnyRenderObject {
    value: ClonableOpaque
}

pub enum ObjectAction {
    Add {
        id: u64,
        parent_id: u64,
        data: AnyRenderObject
    },
    Remove {
        id: u64,
        parent_id: u64,
    }
}

pub trait RenderObject
where
    Self: Clone + Sized + 'static,
{
    fn into_any(self) -> AnyRenderObject {
        AnyRenderObject::new(self)
    }
}


impl AnyRenderObject {
    pub(crate) fn new<T: Clone + Any>(value: T) -> Self {
        Self {
            value: ClonableOpaque::new(value)
        }
    }

    pub fn downcast_ref<T: Clone + Any>(&self) -> &T {
        self.value.downcast_ref::<T>()
    }

    pub fn downcast<T: Clone + Any>(self) -> T {
        self.value.downcast::<T>()
    }
}

