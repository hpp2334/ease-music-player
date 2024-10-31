use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    hash::Hash,
    ops::DerefMut,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use async_task::{Runnable, Task};
use futures::future::LocalBoxFuture;

pub trait IAsyncRuntimeAdapter: Send + Sync + 'static {
    fn on_schedule(&self);
    fn sleep(&self, duration: Duration) -> LocalBoxFuture<'static, ()>;
    fn get_time(&self) -> Duration;
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct AsyncTaskId(u64);

#[derive(Default, Clone)]
pub struct AsyncTasks {
    tasks: Rc<RefCell<HashMap<AsyncTaskId, Task<()>>>>,
    id_allocator: Arc<AtomicU64>,
}

pub struct AsyncExecutor {
    adapter: Arc<dyn IAsyncRuntimeAdapter>,
    runnable_sender: flume::Sender<Runnable>,
    runnable_receiver: flume::Receiver<Runnable>,
}

impl AsyncTasks {
    pub(crate) fn allocate(&self) -> AsyncTaskId {
        let id = self
            .id_allocator
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        AsyncTaskId(id)
    }

    pub(crate) fn bind(&self, id: AsyncTaskId, task: Task<()>) {
        let old = self.tasks.borrow_mut().insert(id, task);
        assert!(old.is_none());
    }

    pub fn cancel_all(&self) {
        std::mem::swap(self.tasks.borrow_mut().deref_mut(), &mut HashMap::new());
    }

    pub fn cancel(&self, id: AsyncTaskId) {
        self.tasks.borrow_mut().remove(&id);
    }

    pub(crate) fn remove(&self, id: AsyncTaskId) {
        self.tasks.borrow_mut().remove(&id);
    }
}

impl AsyncExecutor {
    pub fn new(adapter: impl IAsyncRuntimeAdapter) -> Self {
        let (tx, rx) = flume::unbounded();

        Self {
            adapter: Arc::new(adapter),
            runnable_sender: tx,
            runnable_receiver: rx,
        }
    }

    pub(crate) fn spawn_local<Fut>(&self, fut: Fut) -> (Runnable, Task<()>)
    where
        Fut: Future<Output = ()> + 'static,
    {
        let sender = self.runnable_sender.clone();
        let adapter = self.adapter.clone();

        let schedule = move |runnable| {
            sender.send(runnable).unwrap();
            adapter.on_schedule();
        };
        async_task::spawn_local(fut, schedule)
    }

    pub async fn sleep(&self, duration: Duration) {
        self.adapter.sleep(duration).await
    }

    pub fn get_time(&self) -> Duration {
        self.adapter.get_time()
    }

    pub(crate) fn flush_runnables(&self) -> bool {
        if self.runnable_receiver.is_empty() {
            return false;
        }

        while let Ok(runnable) = self.runnable_receiver.try_recv() {
            runnable.run();
        }
        return true;
    }
}

pub(crate) struct DefaultAsyncRuntimeAdapter;

impl IAsyncRuntimeAdapter for DefaultAsyncRuntimeAdapter {
    fn on_schedule(&self) {
        panic!("async runtime adapter not registered")
    }

    fn sleep(&self, _duration: std::time::Duration) -> LocalBoxFuture<'static, ()> {
        panic!("async runtime adapter not registered")
    }

    fn get_time(&self) -> std::time::Duration {
        panic!("async runtime adapter not registered")
    }
}
