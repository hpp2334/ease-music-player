use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, RwLock,
    },
    time::Duration,
};

use async_task::{Runnable, Task};

pub use futures::future::{BoxFuture, LocalBoxFuture};

enum LocalTask {
    Runnable(Runnable),
    Callback(Box<dyn FnOnce() + Send + Sync + 'static>),
}

pub trait IAsyncRuntimeAdapter: Send + Sync + 'static {
    fn is_main_thread(&self) -> bool;
    fn on_spawn_locals(&self);
    fn sleep(&self, duration: Duration) -> LocalBoxFuture<()>;
    fn get_time(&self) -> Duration;
}

pub trait IOnAsyncRuntime: Send + Sync + 'static {
    fn flush_spawned_locals(&self);
}

pub struct AsyncRuntime {
    adapter: Arc<dyn IAsyncRuntimeAdapter>,
    locals_sender: flume::Sender<LocalTask>,
    locals_receiver: flume::Receiver<LocalTask>,
    local_notified: Arc<AtomicBool>,
}

impl AsyncRuntime {
    pub fn new(adapter: Arc<dyn IAsyncRuntimeAdapter>) -> Arc<Self> {
        let (tx, rx) = flume::unbounded();

        Arc::new(Self {
            adapter,
            locals_sender: tx,
            locals_receiver: rx,
            local_notified: Default::default(),
        })
    }

    pub fn schedule_main(self: &Arc<Self>, f: impl FnOnce() + Send + Sync + 'static) {
        self.locals_sender
            .send(LocalTask::Callback(Box::new(f)))
            .unwrap();

        if !self
            .local_notified
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            self.adapter.on_spawn_locals();
        }
    }

    pub fn spawn<Fut>(self: &Arc<Self>, fut: Fut) -> Task<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        let schedule = move |runnable: Runnable| {
            tokio::spawn(async move {
                runnable.run();
            });
        };
        let (runnable, task) = async_task::spawn(fut, schedule);
        runnable.schedule();
        task
    }

    pub fn spawn_local_runnable<Fut>(self: &Arc<Self>, fut: Fut) -> (Runnable, Task<Fut::Output>)
    where
        Fut: Future + 'static,
        Fut::Output: 'static,
    {
        assert!(self.adapter.is_main_thread());
        let sender = self.locals_sender.clone();
        let adapter = self.adapter.clone();
        let local_notified = self.local_notified.clone();

        let schedule = move |runnable| {
            sender.send(LocalTask::Runnable(runnable)).unwrap();

            if !local_notified.swap(true, std::sync::atomic::Ordering::Relaxed) {
                adapter.on_spawn_locals();
            }
        };
        async_task::spawn_local(fut, schedule)
    }

    pub fn spawn_local<Fut>(self: &Arc<Self>, fut: Fut) -> Task<Fut::Output>
    where
        Fut: Future + 'static,
        Fut::Output: 'static,
    {
        let (runnable, task) = self.spawn_local_runnable(fut);
        runnable.schedule();
        task
    }

    pub fn sleep(self: &Arc<Self>, duration: Duration) -> LocalBoxFuture<()> {
        self.adapter.sleep(duration)
    }

    pub fn get_time(self: &Arc<Self>) -> Duration {
        self.adapter.get_time()
    }

    pub fn flush_local_spawns(self: &Arc<Self>) -> bool {
        assert!(self.adapter.is_main_thread());
        self.local_notified
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if self.locals_receiver.is_empty() {
            return false;
        }

        while let Ok(local) = self.locals_receiver.try_recv() {
            match local {
                LocalTask::Callback(callback) => {
                    callback();
                }
                LocalTask::Runnable(runnable) => {
                    runnable.run();
                }
            }
        }

        return true;
    }
}
