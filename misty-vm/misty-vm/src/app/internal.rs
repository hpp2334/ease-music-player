use std::{
    future::Future,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
    time::Duration,
};

use crate::{
    async_task::{AsyncTasks, IAsyncRuntimeAdapter},
    models::Models,
    to_host::ToHosts,
    view_models::BoxedViewModels,
    IToHost, Model, ViewModelContext,
};

pub(crate) struct AppInternal {
    pub models: Models,
    pub view_models: Box<dyn BoxedViewModels>,
    pub to_hosts: ToHosts,
    pub async_tasks: AsyncTasks,
}

impl AppInternal {
    pub fn emit<Event>(self: &Arc<AppInternal>, evt: Event)
    where
        Event: 'static,
    {
        self.view_models.handle_event(self, Box::new(evt));
    }

    pub fn enqueue_emit<Event>(self: &Arc<AppInternal>, evt: Event)
    where
        Event: 'static,
    {
        let app = self.clone();
        self.async_tasks.spawn_local(async move {
            app.emit(evt);
        });
    }

    pub fn start(self: &Arc<AppInternal>) {
        let cx = ViewModelContext::new(self.clone());
        self.view_models.handle_start(&cx);
    }

    pub fn model_get<T>(&self) -> std::cell::Ref<'_, T>
    where
        T: 'static,
    {
        self.models.read()
    }

    pub fn model_mut<T>(&self) -> std::cell::RefMut<'_, T>
    where
        T: 'static,
    {
        self.models.read_mut()
    }

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        self.to_hosts.get::<C>()
    }

    pub fn async_tasks(&self) -> &AsyncTasks {
        &self.async_tasks
    }
}
