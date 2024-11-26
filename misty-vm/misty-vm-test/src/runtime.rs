use std::{
    sync::{
        atomic::AtomicBool,
        Arc, Mutex,
    },
    thread::ThreadId,
    time::Duration,
};

use misty_vm::{BoxFuture, IAsyncRuntimeAdapter, IOnAsyncRuntime};

use crate::timer::FakeTimers;

struct AsyncRuntimeAdapterInternal {
    connected: Arc<Mutex<Option<Arc<dyn IOnAsyncRuntime>>>>,
    thread_id: ThreadId,
    notified: AtomicBool,
    timers: FakeTimers,
}

#[derive(Clone)]
pub struct TestAsyncRuntimeAdapter {
    store: Arc<AsyncRuntimeAdapterInternal>,
}
unsafe impl Send for TestAsyncRuntimeAdapter {}
unsafe impl Sync for TestAsyncRuntimeAdapter {}

impl TestAsyncRuntimeAdapter {
    pub fn new() -> Self {
        Self {
            store: Arc::new(AsyncRuntimeAdapterInternal {
                connected: Default::default(),
                thread_id: std::thread::current().id(),
                notified: Default::default(),
                timers: FakeTimers::new(),
            }),
        }
    }

    fn connector(&self) -> Arc<dyn IOnAsyncRuntime> {
        let w = self.store.connected.lock().unwrap();
        w.clone().unwrap()
    }

    pub fn bind(&self, connector: Arc<dyn IOnAsyncRuntime>) {
        self.check_same_thread();
        {
            let mut w = self.store.connected.lock().unwrap();
            *w = Some(connector);
        }
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
        let app = self.connector();
        let mut count = 0;

        loop {
            let notified = self
                .store
                .notified
                .swap(false, std::sync::atomic::Ordering::Relaxed);
            if !notified {
                break;
            }
            if count == 100 {
                panic!("too many flush")
            }

            app.flush_spawned_locals();
            count += 1;
        }
    }

    fn is_same_thread(&self) -> bool {
        self.store.thread_id == std::thread::current().id()
    }

    fn check_same_thread(&self) {
        assert!(
            self.is_same_thread(),
            "Operation must be performed on the same thread"
        );
    }
}

impl IAsyncRuntimeAdapter for TestAsyncRuntimeAdapter {
    fn is_main_thread(&self) -> bool {
        self.is_same_thread()
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.check_same_thread();
        let timer = self.store.timers.sleep(duration);
        Box::pin(timer)
    }

    fn get_time(&self) -> std::time::Duration {
        self.check_same_thread();
        self.store.timers.get_current_time()
    }

    fn on_spawn_locals(&self) {
        self.store
            .notified
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
