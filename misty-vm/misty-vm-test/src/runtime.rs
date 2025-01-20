use std::{
    sync::{atomic::AtomicBool, Arc, Mutex, Weak},
    thread::ThreadId,
    time::Duration,
};

use misty_vm::{BoxFuture, ILifecycleExternal};

use crate::timer::FakeTimers;

struct Internal {
    thread_id: ThreadId,
    timers: FakeTimers,
}

#[derive(Clone)]
pub struct TestLifecycleExternal {
    store: Arc<Internal>,
}
unsafe impl Send for TestLifecycleExternal {}
unsafe impl Sync for TestLifecycleExternal {}

impl TestLifecycleExternal {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Internal {
                thread_id: std::thread::current().id(),
                timers: FakeTimers::new(),
            }),
        }
    }

    pub fn advance(&self, duration: Duration) {
        const MILLIS: u64 = 500;

        self.check_same_thread();

        let step = Duration::from_millis(MILLIS);
        let mut remaining = duration;
        while remaining > Duration::ZERO {
            let advance_duration = if remaining < step { remaining } else { step };

            self.advance_impl(advance_duration);
            remaining -= advance_duration;
        }
        self.advance_impl(Duration::ZERO);
    }

    fn advance_impl(&self, duration: Duration) {
        self.store.timers.advance(duration);
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

impl ILifecycleExternal for TestLifecycleExternal {
    fn get_time(&self) -> std::time::Duration {
        self.check_same_thread();
        self.store.timers.get_current_time()
    }

    fn is_main_thread(&self) -> bool {
        self.store.thread_id == std::thread::current().id()
    }

    fn spawn_main_thread(&self, runnable: misty_vm::Runnable) {
        self.store.timers.sleep(Duration::from_millis(0), move || {
            runnable.run();
        });
    }

    fn spawn(&self, runnable: misty_vm::Runnable) {
        tokio::spawn(async move {
            runnable.run();
        });
    }

    fn spawn_sleep(&self, duration: Duration, runnable: misty_vm::Runnable) {
        self.store.timers.sleep(duration, move || {
            runnable.run();
        });
    }
}
