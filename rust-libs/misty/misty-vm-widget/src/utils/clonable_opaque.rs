use std::any::{Any, TypeId};

trait ClonableOpaqueValue: Any {
    fn clone_box(&self) -> Box<dyn ClonableOpaqueValue>;
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T> ClonableOpaqueValue for T
where
    T: Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableOpaqueValue> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub(crate) struct ClonableOpaque {
    type_id: TypeId,
    value: Box<dyn ClonableOpaqueValue>,
}

impl ClonableOpaque {
    pub fn new<T: Clone + Any>(value: T) -> Self {
        Self {
            type_id: std::any::TypeId::of::<T>(),
            value: Box::new(value),
        }
    }

    pub fn downcast_ref<T: Clone + Any>(&self) -> &T {
        self.check_same_type::<T>();
        self.value.as_any()
            .downcast_ref::<T>()
            .expect("fail to ClonableOpaque downcast_ref")
    }

    pub fn downcast<T: Clone + Any>(self) -> T {
        self.check_same_type::<T>();
        let value = self.value;
        let v = value.into_any();
        *v.downcast::<T>().expect("fail to ClonableOpaque downcast")
    }

    fn check_same_type<T>(&self)
    where T: 'static {
        let to = std::any::TypeId::of::<T>();
        if to != self.type_id {
            panic!("Type is {:?}, but will cast to {:?}", self.type_id, to);
        }
    }
}

impl Clone for ClonableOpaque {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            value: self.value.clone_box(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ClonableOpaque;

    #[test]
    fn test_i32() {
        let v = ClonableOpaque::new(3i32);
        let v = v.downcast_ref::<i32>();
        assert_eq!(*v, 3);
    }
}
