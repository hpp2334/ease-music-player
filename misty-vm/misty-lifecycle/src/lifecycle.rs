use std::{
    future::Future,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use async_task::{Runnable, Task};
use futures::future::BoxFuture;

pub trait ILifecycleExternal: Send + Sync + 'static {
    fn is_main_thread(&self) -> bool;
    fn spawn_main_thread(&self, runnable: Runnable);
    fn spawn(&self, runnable: Runnable);
    fn spawn_sleep(&self, duration: Duration, runnable: Runnable);
    fn get_time(&self) -> Duration;
}

pub struct Lifecycle {
    dispatcher: Arc<dyn ILifecycleExternal>,
}

impl Lifecycle {
    pub fn new(dispatcher: Arc<dyn ILifecycleExternal>) -> Arc<Self> {
        Arc::new(Self { dispatcher })
    }

    pub fn spawn<Fut>(self: &Arc<Self>, fut: Fut) -> Task<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        let dispatcher = self.dispatcher.clone();
        let schedule = move |runnable: Runnable| {
            dispatcher.spawn(runnable);
        };
        let (runnable, task) = async_task::spawn(fut, schedule);
        runnable.schedule();
        task
    }

    pub fn spawn_main_thread<Fut>(self: &Arc<Self>, fut: Fut) -> Task<Fut::Output>
    where
        Fut: Future + 'static,
        Fut::Output: 'static,
    {
        assert!(self.dispatcher.is_main_thread());
        let dispatcher = self.dispatcher.clone();
        let schedule = move |runnable: Runnable| {
            dispatcher.spawn_main_thread(runnable);
        };
        let (runnable, task) = async_task::spawn_local(fut, schedule);
        runnable.schedule();
        task
    }

    pub fn sleep(self: &Arc<Self>, duration: Duration) -> BoxFuture<()> {
        let dispatcher = self.dispatcher.clone();
        let schedule = move |runnable: Runnable| {
            dispatcher.spawn_sleep(duration, runnable);
        };
        let (runnable, task) = async_task::spawn(async {}, schedule);
        runnable.schedule();
        Box::pin(async move { task.await })
    }

    pub fn get_time(self: &Arc<Self>) -> Duration {
        self.dispatcher.get_time()
    }
}
