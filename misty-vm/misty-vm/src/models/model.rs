use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, RwLock},
};

pub(crate) struct AnyModel {
    value: Box<Rc<RefCell<dyn Any>>>,
}

impl AnyModel {
    pub fn new<T>() -> Self
    where
        T: Default + 'static,
    {
        AnyModel {
            value: Box::new(Rc::new(RefCell::new(T::default()))),
        }
    }

    pub fn read<T>(&self) -> Ref<'_, T>
    where
        T: 'static,
    {
        Ref::map(self.value.borrow(), |v| v.downcast_ref::<T>().unwrap())
    }

    pub fn read_mut<T>(&self) -> RefMut<'_, T>
    where
        T: 'static,
    {
        RefMut::map(self.value.borrow_mut(), |v| v.downcast_mut::<T>().unwrap())
    }
}

pub(super) struct RawModels {
    models: HashMap<TypeId, AnyModel>,
}

impl RawModels {
    pub fn new() -> Self {
        Self {
            models: Default::default(),
        }
    }

    pub fn insert<T>(&mut self)
    where
        T: 'static + Default,
    {
        let id = std::any::TypeId::of::<T>();
        self.models.insert(id, AnyModel::new::<T>());
    }

    pub fn read<T>(&self) -> &AnyModel
    where
        T: 'static,
    {
        let id = std::any::TypeId::of::<T>();
        self.models.get(&id).expect(
            format!(
                "[Internal Error] model {} not registered",
                std::any::type_name::<T>()
            )
            .as_str(),
        )
    }
}
