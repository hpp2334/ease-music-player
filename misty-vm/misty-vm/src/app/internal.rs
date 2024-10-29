use std::{
    any::Any, future::Future, ops::Deref, sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    }, thread::ThreadId, time::Duration
};

use crate::{
    async_task::{AsyncExecutor, IAsyncRuntimeAdapter}, models::Models, to_host::ToHosts, BoxedViewModels, IToHost, Model, ViewModelContext
};

pub(crate) struct AppInternal {
    pub thread_id: ThreadId,
    pub models: Models,
    pub view_models: BoxedViewModels,
    pub to_hosts: ToHosts,
    pub async_executor: AsyncExecutor,
}

impl AppInternal {
    pub fn emit<Event, E>(self: &Arc<AppInternal>, evt: Event)
    where Event: Any + 'static,
    E: Any + 'static,
    {
        self.view_models.handle_event::<Event, E>(self, evt);
    }

    pub fn enqueue_emit<Event, E>(self: &Arc<AppInternal>, evt: Event)
    where Event: Any + 'static,
    E: Any + 'static,
    {
        let app = self.clone();
        let (runnable, task) = self.async_executor.spawn_local(async move {
            app.emit::<Event, E>(evt);
        });
        runnable.schedule();
        task.detach();
    }
}
