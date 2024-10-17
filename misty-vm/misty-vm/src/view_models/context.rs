use std::{future::Future, sync::Arc, time::Duration};

use crate::{internal::AppInternal, utils::PhantomUnsend, IToHost, Model};

pub struct ViewModelContext {
    _app: Arc<AppInternal>,
    _unsend: PhantomUnsend,
}

#[derive(Clone)]
pub struct WeakViewModelContext {
    _app: Arc<AppInternal>,
}

impl ViewModelContext {
    pub(crate) fn new(app: Arc<AppInternal>) -> Self {
        Self {
            _app: app,
            _unsend: Default::default(),
        }
    }

    pub fn weak(&self) -> WeakViewModelContext {
        WeakViewModelContext {
            _app: self._app.clone(),
        }
    }

    pub fn model_get<T>(&self, model: &Model<T>) -> std::cell::Ref<'_, T>
    where
        T: 'static,
    {
        self._app.model_get()
    }

    pub fn model_mut<T>(&self, model: &Model<T>) -> std::cell::RefMut<'_, T>
    where
        T: 'static,
    {
        self._app.model_mut()
    }

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        self._app.to_host::<C>()
    }

    pub fn spawn<F, Fut>(&self, f: F)
    where
        F: FnOnce(ViewModelContext) -> Fut,
        Fut: Future<Output = ()> + 'static,
    {
        let fut = f(self.clone_internal());
        self._app.async_tasks().spawn_local(fut);
    }

    pub async fn sleep(&self, duration: Duration) {
        self._app.async_tasks().sleep(duration).await
    }

    pub fn enqueue_emit<Event>(&self, evt: Event)
    where
        Event: 'static,
    {
        self._app.enqueue_emit(evt);
    }

    fn clone_internal(&self) -> Self {
        Self {
            _app: self._app.clone(),
            _unsend: Default::default(),
        }
    }
}

impl WeakViewModelContext {
    pub fn upgrade(&self) -> ViewModelContext {
        ViewModelContext {
            _app: self._app.clone(),
            _unsend: Default::default(),
        }
    }
}
