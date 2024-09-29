use std::{any::{Any, TypeId}, cell::{Ref, RefCell, RefMut}, marker::PhantomData, rc::Rc};

use super::WidgetContext;


#[derive(Clone)]
pub struct AnyWidgetState {
    type_id: TypeId,
    value: Rc<RefCell<dyn Any>>,
}

pub struct WidgetState<T> {
    state: AnyWidgetState,
    _marker: PhantomData<T>,
}

impl AnyWidgetState {
    pub fn new<T>(value: T) -> Self
    where T: 'static {
        Self {
            type_id: std::any::TypeId::of::<T>(),
            value: Rc::new(RefCell::new(value)),
        }
    }

    pub fn downcast_ref<T>(&self) -> Ref<'_, T>
    where T: Any + 'static {
        Ref::map(self.value.borrow(), |v| v.downcast_ref::<T>().expect("fail to AnyWidgetState downcast"))
    }

    pub fn downcast_mut<T>(&self) -> RefMut<'_, T>
    where T: Any + 'static {
        RefMut::map(self.value.borrow_mut(), |v| v.downcast_mut::<T>().expect("fail to AnyWidgetState downcast_mut"))
    }
}

impl<T> WidgetState<T>
where
    T: 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            state: AnyWidgetState::new(value),
            _marker: Default::default(),
        }
    }
    pub fn wrap(value: AnyWidgetState) -> Self {
        assert_eq!(value.type_id, std::any::TypeId::of::<T>());

        Self {
            state: value,
            _marker: Default::default(),
        }
    }

    pub fn get(&self, cx: &WidgetContext) -> Ref<'_, T> {
        self.state.downcast_ref::<T>()
    }
    pub fn get_mut(&self, cx: &WidgetContext) -> RefMut<'_, T> {
        self.state.downcast_mut::<T>()
    }
}
