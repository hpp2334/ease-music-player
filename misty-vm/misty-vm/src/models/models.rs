use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    collections::{HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

use super::model::RawModels;

pub struct Model<T> {
    _marker: PhantomData<T>,
}

pub(crate) struct Models {
    raw: RawModels,
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
        let model = self.raw.read::<T>();
        model.read_mut::<T>()
    }
}
