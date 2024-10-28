use std::{
    future::Future,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
    thread::ThreadId,
    time::Duration,
};

use crate::{
    async_task::{AsyncExecutor, IAsyncRuntimeAdapter},
    models::Models,
    to_host::ToHosts,
    view_models::BoxedViewModels,
    IToHost, Model, ViewModelContext,
};

pub(crate) struct AppInternal {
    pub thread_id: ThreadId,
    pub models: Models,
    pub view_models: Box<dyn BoxedViewModels>,
    pub to_hosts: ToHosts,
    pub async_executor: AsyncExecutor,
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
        let (runnable, task) = self.async_executor.spawn_local(async move {
            app.emit(evt);
        });
        runnable.schedule();
        task.detach();
    }
}
