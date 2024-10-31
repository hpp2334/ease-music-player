use std::{
    any::Any,
    fmt::Debug,
    future::Future,
    sync::Arc,
    time::Duration,
};

use crate::{
    async_task::{AsyncTaskId, AsyncTasks},
    internal::AppInternal,
    utils::PhantomUnsend,
    IToHost, IntoVMError, Model,
};

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

    pub(crate) fn app(&self) -> &AppInternal {
        &self._app
    }

    pub fn model_get<T>(&self, _model: &Model<T>) -> std::cell::Ref<'_, T>
    where
        T: 'static,
    {
        self._app.models.read()
    }

    pub fn model_mut<T>(&self, _model: &Model<T>) -> std::cell::RefMut<'_, T>
    where
        T: 'static,
    {
        self._app.models.read_mut()
    }

    pub fn model_dirty<T>(&self, _model: &Model<T>) -> bool
    where
        T: 'static,
    {
        self._app.models.is_dirty::<T>()
    }

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        self._app.to_hosts.get::<C>()
    }

    pub fn spawn<F, Fut, E>(&self, tasks: &AsyncTasks, f: F) -> AsyncTaskId
    where
        F: FnOnce(ViewModelContext) -> Fut,
        Fut: Future<Output = Result<(), E>> + 'static,
        E: IntoVMError,
    {
        let fut = f(self.clone_internal());
        let id = tasks.allocate();
        let (runnable, raw_task) = {
            let tasks = tasks.clone();
            self._app.async_executor.spawn_local(async move {
                let r = fut.await;
                tasks.remove(id);
                if let Err(e) = r {
                    panic!("spawn error: {}", e.cast());
                }
            })
        };
        tasks.bind(id, raw_task);
        runnable.schedule();
        id
    }

    pub async fn sleep(&self, duration: Duration) {
        self._app.async_executor.sleep(duration).await
    }

    pub fn get_time(&self) -> Duration {
        self._app.async_executor.get_time()
    }

    pub fn enqueue_emit<Event>(&self, evt: Event)
    where
        Event: Any + Debug + 'static,
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
