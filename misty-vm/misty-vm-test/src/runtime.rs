use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
    time::{Duration, Instant},
};

use misty_vm::IAsyncRuntimeAdapter;

use crate::timer::FakeTimers;

struct AsyncRuntimeInternal {
    runtime: tokio::runtime::Runtime,
    local: RefCell<tokio::task::LocalSet>,
    timers: FakeTimers,
    id_alloc: AtomicU64,
}

#[derive(Clone)]
pub struct AsyncRuntime {
    store: Arc<AsyncRuntimeInternal>,
}
impl AsyncRuntime {
    pub fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();

        Self {
            store: Arc::new(AsyncRuntimeInternal {
                runtime: rt,
                local: RefCell::new(local),
                timers: FakeTimers::new(),
                id_alloc: AtomicU64::new(0),
            }),
        }
    }

    pub fn enter(&self) -> tokio::runtime::EnterGuard<'_> {
        self.store.runtime.enter()
    }

    pub fn advance(&self, duration: Duration) {
        const MILLIS: u64 = 500;

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
        self.store.runtime.block_on(async move {
            store
                .local
                .borrow()
                .run_until(async move {
                    self.store
                        .local
                        .borrow()
                        .spawn_local(async move {
                        })
                        .await
                        .unwrap();
                })
                .await;
        });
    }
}

impl IAsyncRuntimeAdapter for AsyncRuntime {
    fn spawn_local(&self, future: misty_vm::LocalBoxFuture<'static, ()>) -> u64 {
        self.store.local.borrow().spawn_local(future);
        let id = self
            .store
            .id_alloc
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        id
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
        let timer = self.store.timers.sleep(duration);
        Box::pin(timer)
    }

    fn get_time(&self) -> std::time::Duration {
        self.store.timers.get_current_time()
    }
}
