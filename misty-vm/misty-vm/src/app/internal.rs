use std::{
    any::Any,
    fmt::Debug,
    sync::{atomic::AtomicBool, Arc, Weak},
    thread::ThreadId,
};

use misty_async::AsyncRuntime;

use crate::{models::Models, to_host::ToHosts, view_models::pod::ViewModels};

pub(crate) struct AppInternal {
    pub thread_id: ThreadId,
    pub models: Models,
    pub view_models: ViewModels,
    pub to_hosts: ToHosts,
    pub async_executor: Arc<AsyncRuntime>,
    pub during_flush: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct WeakAppInternal {
    pub internal: Weak<AppInternal>,
}
unsafe impl Send for WeakAppInternal {}
unsafe impl Sync for WeakAppInternal {}

impl WeakAppInternal {
    pub fn new(app: &Arc<AppInternal>) -> Self {
        Self {
            internal: Arc::downgrade(app),
        }
    }

    pub fn upgrade(&self) -> Option<Arc<AppInternal>> {
        let app = self.internal.upgrade();
        if let Some(app) = app {
            app.check_same_thread();
            Some(app)
        } else {
            None
        }
    }
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
        self.before_flush_emit(&evt);
        self.view_models.handle_event(self, evt);
        self.after_flush();
    }

    pub fn flush_spawned(self: &Arc<Self>) {
        self.check_same_thread();
        let should_flush = self.async_executor.flush_local_spawns();
        if should_flush {
            self.before_flush_flush_spawned();
            self.view_models.handle_flush(self);
            self.after_flush();
        }
    }

    pub fn enqueue_emit<Event>(self: &Arc<Self>, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        let app = self.clone();
        let (runnable, task) = self.async_executor.spawn_local_runnable(async move {
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

    fn before_flush_emit<Event>(&self, e: &Event)
    where
        Event: Debug,
    {
        let lock = self
            .during_flush
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if lock {
            panic!(
                "emit Event {:?}, but ViewModels are during on_event or on_flush",
                e
            )
        }
    }

    fn before_flush_flush_spawned(&self) {
        let lock = self
            .during_flush
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if lock {
            panic!("flush_spawned, but ViewModels are during on_event or on_flush")
        }
    }

    fn after_flush(&self) {
        self.during_flush
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.models.clear_dirties();
    }
}
