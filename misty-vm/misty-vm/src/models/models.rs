use std::{
    any::TypeId,
    cell::{Ref, RefCell, RefMut},
    collections::HashSet,
    marker::PhantomData,
};

use super::model::RawModels;

pub struct Model<T> {
    _marker: PhantomData<T>,
}

pub(crate) struct Models {
    raw: RawModels,
    dirties: RefCell<HashSet<TypeId>>,
}

impl<T> Model<T> {
    fn new() -> Self {
        Self {
            _marker: Default::default(),
        }
    }
}

impl<T> Clone for Model<T> {
    fn clone(&self) -> Self {
        Self {
            _marker: Default::default(),
        }
    }
}

impl Models {
    pub fn new() -> Self {
        Self {
            raw: RawModels::new(),
            dirties: Default::default(),
        }
    }

    pub fn insert<T>(&mut self) -> Model<T>
    where
        T: 'static + Default,
    {
        self.raw.insert::<T>();

        Model::new()
    }

    pub fn read<T>(&self) -> Ref<'_, T>
    where
        T: 'static,
    {
        let model = self.raw.read::<T>();
        model.read::<T>()
    }

    pub fn read_mut<T>(&self) -> RefMut<'_, T>
    where
        T: 'static,
    {
        self.mark_dirty::<T>();
        let model = self.raw.read::<T>();
        model.read_mut::<T>()
    }

    pub fn is_dirty<T>(&self) -> bool
    where
        T: 'static,
    {
        self.dirties.borrow().contains(&TypeId::of::<T>())
    }

    pub fn clear_dirties(&self) {
        self.dirties.borrow_mut().clear();
    }

    fn mark_dirty<T>(&self)
    where
        T: 'static,
    {
        self.dirties.borrow_mut().insert(TypeId::of::<T>());
    }
}
