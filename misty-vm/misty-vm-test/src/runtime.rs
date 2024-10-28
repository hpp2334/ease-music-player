use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
    thread::ThreadId,
    time::{Duration, Instant},
};

use misty_vm::{App, AppPod, BoxFuture, IAsyncRuntimeAdapter};

use crate::timer::FakeTimers;

struct AsyncRuntimeInternal {
    runtime: tokio::runtime::Runtime,
    timers: FakeTimers,
    pod: AppPod,
    thread_id: ThreadId,
}

#[derive(Clone)]
pub struct AsyncRuntime {
    store: Arc<AsyncRuntimeInternal>,
}
unsafe impl Send for AsyncRuntime {}
unsafe impl Sync for AsyncRuntime {}

impl AsyncRuntime {
    pub fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        Self {
            store: Arc::new(AsyncRuntimeInternal {
                runtime: rt,
                timers: FakeTimers::new(),
                pod: Default::default(),
                thread_id: std::thread::current().id(),
            }),
        }
    }

    pub fn bind_app(&self, app: App) {
        self.check_same_thread();
        self.store.pod.set(app);
    }

    pub fn enter(&self) -> tokio::runtime::EnterGuard<'_> {
        self.check_same_thread();
        self.store.runtime.enter()
    }

    pub fn advance(&self, duration: Duration) {
        const MILLIS: u64 = 500;

        self.check_same_thread();
        self.wait_all();

        let step = Duration::from_millis(MILLIS);
        let mut remaining = duration;
        while remaining > Duration::ZERO {
            let advance_duration = if remaining < step { remaining } else { step };

            self.advance_impl(advance_duration);
            remaining -= advance_duration;
        }
    }

    fn advance_impl(&self, duration: Duration) {
        self.store.timers.advance(duration);
        self.wait_all();
    }

    fn wait_all(&self) {
        let store = self.store.clone();
        let app = store.pod.get();
        app.flush_spawned();
    }

    fn check_same_thread(&self) {
        assert_eq!(
            self.store.thread_id,
            std::thread::current().id(),
            "Operation must be performed on the same thread"
        );
    }
}

impl IAsyncRuntimeAdapter for AsyncRuntime {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
        self.check_same_thread();
        let timer = self.store.timers.sleep(duration);
        Box::pin(timer)
    }

    fn get_time(&self) -> std::time::Duration {
        self.check_same_thread();
        self.store.timers.get_current_time()
    }

    fn on_schedule(&self) {
        // noop
    }
}
