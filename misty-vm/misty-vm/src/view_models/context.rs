use std::{any::Any, future::Future, rc::Rc, sync::Arc, time::Duration};

use crate::{async_task::AsyncTasks, internal::AppInternal, utils::PhantomUnsend, IToHost, Model};

use super::ViewModel;

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

    pub(crate) fn vm<V, Event, E>(&self) -> Rc<V>
    where
        E: Any + 'static,
        V: ViewModel<Event, E>,
    {
        todo!()
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
    T: 'static {
        todo!()
    }

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        self._app.to_hosts.get::<C>()
    }

    pub fn spawn<F, Fut, E>(&self, tasks: &AsyncTasks, f: F)
    where
        F: FnOnce(ViewModelContext) -> Fut,
        Fut: Future<Output = Result<(), E>> + 'static,
    {
        let fut = f(self.clone_internal());
        let id = tasks.allocate();
        let (runnable, raw_task) = {
            let tasks= tasks.clone();
            self._app.async_executor.spawn_local(async move {
                let r = fut.await;
                tasks.remove(id);
                if let Err(e) = r {
                    tracing::error!("spawn error");
                }
            })
        };
        tasks.bind(id, raw_task);
        runnable.schedule();
    }

    pub async fn sleep(&self, duration: Duration) {
        self._app.async_executor.sleep(duration).await
    }

    pub fn get_time(&self) -> Duration {
        self._app.async_executor.get_time()
    }

    pub fn enqueue_emit<Event, E>(&self, evt: Event)
    where
        Event: Any + 'static,
        E: Any + 'static,
    {
        self._app.enqueue_emit::<Event, E>(evt);
    }

    fn clone_internal(&self) -> Self {
        Self {
            _app: self._app.clone(),
            _unsend: Default::default(),
        }
    }
}