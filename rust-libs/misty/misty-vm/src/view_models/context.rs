use std::sync::Arc;

use crate::{internal::AppInternal, utils::PhantomUnsend, IToHost, Model};

pub struct ViewModelContext {
    _app: Arc<AppInternal>,
    _unsend: PhantomUnsend,
}

impl ViewModelContext {
    pub(crate) fn new(app: Arc<AppInternal>) -> Self {
        Self {
            _app: app,
            _unsend: Default::default(),
        }
    }

    pub fn update<T>(&self, model: &Model<T>, update: impl FnOnce(&mut T))
    where
        T: 'static,
    {
        self._app.update_model(model, update);
    }

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost {
        self._app.to_host::<C>()
    }   
}
