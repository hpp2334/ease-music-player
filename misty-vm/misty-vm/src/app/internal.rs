use std::{
    any::Any,
    fmt::Debug,
    sync::{atomic::AtomicBool, Arc, Weak},
    thread::ThreadId,
};

use misty_lifecycle::{ArcLocalCore, Lifecycle};
use tracing::instrument;

use crate::{models::Models, to_host::ToHosts, view_models::pod::ViewModels};

pub(crate) struct AppInternal {
    pub local: ArcLocalCore,
    pub models: Models,
    pub view_models: ViewModels,
    pub to_hosts: ToHosts,
    pub async_executor: Arc<Lifecycle>,
    pub pending_events: (
        flume::Sender<Box<dyn Any + 'static>>,
        flume::Receiver<Box<dyn Any + 'static>>,
    ),
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
            app.local.check_same_thread();
            Some(app)
        } else {
            None
        }
    }

    pub fn push_pending_event<Event>(&self, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        let app = unsafe { self.upgrade_unchecked() };
        if let Some(app) = app {
            app.push_pending_event(evt);
        }
    }

    unsafe fn upgrade_unchecked(&self) -> Option<Arc<AppInternal>> {
        let app = self.internal.upgrade();
        if let Some(app) = app {
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

impl Drop for AppInternal {
    fn drop(&mut self) {
        tracing::info!("drop AppInternal")
    }
}

impl AppInternal {
    #[instrument]
    pub fn emit<Event>(self: &Arc<Self>, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        self.local.check_same_thread();
        let lock = self
            .during_flush
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if lock {
            panic!(
                "emit Event {:?}, but ViewModels are during on_event or on_flush",
                evt
            )
        }
        tracing::trace!("start {:?}", evt);
        self.view_models.handle_event(self, &evt);
        self.view_models.handle_flush(self);
        self.after_flush();
        tracing::trace!("end");
    }

    #[instrument]
    pub fn flush_pending_events(self: &Arc<Self>) {
        self.local.check_same_thread();
        let len = self.pending_events.1.len();
        if len == 0 {
            return;
        }

        let lock = self
            .during_flush
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if lock {
            panic!("flush_pending_events, but ViewModels are during on_event or on_flush")
        }

        tracing::trace!("start");
        while let Ok(evt) = self.pending_events.1.try_recv() {
            let evt = evt.as_ref();
            self.view_models.handle_event(self, evt);
        }
        self.view_models.handle_flush(self);
        self.after_flush();
        tracing::trace!("end");
    }

    #[instrument]
    pub fn flush_only(self: &Arc<Self>) {
        self.local.check_same_thread();

        let lock = self
            .during_flush
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if lock {
            panic!("flush_only, but ViewModels are during on_event or on_flush")
        }

        tracing::trace!("start");
        self.view_models.handle_flush(self);
        self.after_flush();
        tracing::trace!("end");
    }

    pub fn push_pending_event<Event>(self: &Arc<Self>, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        self.local.debug_check_same_thread();
        let should_schedule = self.pending_events.0.is_empty();
        self.pending_events
            .0
            .send(Box::new(evt))
            .expect("failed to push pending events");

        let weak_internal = WeakAppInternal::new(self);

        if should_schedule {
            self.async_executor
                .spawn_main_thread(async move {
                    if let Some(app) = weak_internal.upgrade() {
                        app.flush_pending_events();
                    }
                })
                .detach();
        }
    }

    pub fn enqueue_emit<Event>(self: &Arc<Self>, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        self.local.debug_check_same_thread();
        self.push_pending_event(evt);
    }

    fn after_flush(&self) {
        self.during_flush
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.models.clear_dirties();
    }
}
