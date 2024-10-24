use std::{any::Any, future::Future, rc::Rc, sync::Arc, time::Duration};

use crate::{internal::AppInternal, utils::PhantomUnsend, IToHost, Model};

use super::ViewModel;

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

    pub(crate) fn app(&self) -> &AppInternal {
        &self._app
    }

    pub fn weak(&self) -> WeakViewModelContext {
        WeakViewModelContext {
            _app: self._app.clone(),
        }
    }

    pub(crate) fn vm<V, Event, E>(&self) -> Rc<V>
    where
        E: Any + 'static,
        V: ViewModel<Event, E>,
    {
        todo!()
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

    pub fn spawn<F, Fut, E>(&self, f: F)
    where
        F: FnOnce(ViewModelContext) -> Fut,
        Fut: Future<Output = Result<(), E>> + 'static,
    {
        let fut = f(self.clone_internal());
        self._app.async_tasks().spawn_local(async move {
            let r = fut.await;
            if let Err(e) = r {
                tracing::error!("spawn error");
            }
        });
    }

    pub fn cancel_spawned(&self) {
        todo!()
    }

    pub async fn sleep(&self, duration: Duration) {
        self._app.async_tasks().sleep(duration).await
    }

    pub fn get_time(&self) -> Duration {
        self._app.async_tasks().get_time()
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
