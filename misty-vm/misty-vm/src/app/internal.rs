use std::{
    any::Any,
    fmt::Debug,
    sync::{
        atomic::AtomicBool,
        Arc,
    },
    thread::ThreadId,
};

use crate::{
    async_task::AsyncExecutor,
    models::Models,
    to_host::ToHosts,
    view_models::pod::ViewModels,
};

pub(crate) struct AppInternal {
    pub thread_id: ThreadId,
    pub models: Models,
    pub view_models: ViewModels,
    pub to_hosts: ToHosts,
    pub async_executor: AsyncExecutor,
    pub during_flush: AtomicBool,
}
impl std::fmt::Debug for AppInternal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppInternal").finish()
    }
}

impl AppInternal {
    pub fn emit<Event>(self: &Arc<Self>, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        self.check_same_thread();
        self.before_flush();
        self.view_models.handle_event(self, evt);
        self.after_flush();
    }

    pub fn flush_spawned(self: &Arc<Self>) {
        self.check_same_thread();
        let should_flush = self.async_executor.flush_runnables();
        if should_flush {
            self.before_flush();
            self.view_models.handle_flush(self);
            self.after_flush();
        }
    }

    pub fn enqueue_emit<Event>(self: &Arc<Self>, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        self.check_same_thread();
        let app = self.clone();
        let (runnable, task) = self.async_executor.spawn_local(async move {
            app.emit(evt);
        });
        runnable.schedule();
        task.detach();
    }

    pub fn check_same_thread(self: &Arc<Self>) {
        let thread_id = std::thread::current().id();
        if self.thread_id != thread_id {
            panic!("cannot operate app in other thread")
        }
    }

    fn before_flush(&self) {
        let lock = self
            .during_flush
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if lock {
            panic!("ViewModels are during on_event or on_flush")
        }
    }

    fn after_flush(&self) {
        self.during_flush
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.models.clear_dirties();
    }
}
