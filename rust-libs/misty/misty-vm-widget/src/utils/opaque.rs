use std::any::{Any, TypeId};

trait OpaqueValue: Any {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T> OpaqueValue for T
where
    T: 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub(crate) struct Opaque {
    type_id: TypeId,
    type_name: String,
    value: Box<dyn OpaqueValue>,
}

impl Opaque {
    pub fn new<T: Any>(value: T) -> Self {
        Self {
            type_id: std::any::TypeId::of::<T>(),
            type_name: std::any::type_name::<T>().to_string(),
            value: Box::new(value),
        }
    }

    pub fn downcast_ref<T: Any>(&self) -> &T {
        self.check_same_type::<T>("downcast_ref");
        self.value.as_any()
            .downcast_ref::<T>()
            .expect("fail to ClonableOpaque downcast_ref")
    }

    pub fn downcast<T: Any>(self) -> T {
        self.check_same_type::<T>("downcast");
        let value = self.value;
        let v = value.into_any();
        *v.downcast::<T>().expect("fail to ClonableOpaque downcast")
    }

    fn check_same_type<T>(&self, scope: &str)
    where T: 'static {
        let to = std::any::TypeId::of::<T>();
        if to != self.type_id {
            let to_type_name = std::any::type_name::<T>();
            panic!("[{}] Type is {:?}, but will cast to {:?}", scope, self.type_name, to_type_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Opaque;

    #[test]
    fn test_i32() {
        let v = Opaque::new(3i32);
        let v = v.downcast_ref::<i32>();
        assert_eq!(*v, 3);
    }
}
