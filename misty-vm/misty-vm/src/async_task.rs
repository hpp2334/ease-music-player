use std::{
    future::Future, time::Duration}
;

use futures::future::LocalBoxFuture;

pub trait IAsyncRuntimeAdapter: 'static {
    fn spawn_local(&self, future: LocalBoxFuture<'static, ()>) -> u64;
    fn sleep(&self, duration: std::time::Duration) -> LocalBoxFuture<'static, ()>;
    fn get_time(&self) -> std::time::Duration;
}

pub struct AsyncTasks {
    adapter: Box<dyn IAsyncRuntimeAdapter>,
}

impl AsyncTasks {
    pub fn new(adapter: impl IAsyncRuntimeAdapter) -> Self {
        Self {
            adapter: Box::new(adapter),
        }
    }

    pub fn spawn_local<Fut>(&self, fut: Fut)
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.adapter.spawn_local(Box::pin(fut));
    }

    pub async fn sleep(&self, duration: Duration) {
        self.adapter.sleep(duration).await
    }
}

pub(crate) struct DefaultAsyncRuntimeAdapter;

impl IAsyncRuntimeAdapter for DefaultAsyncRuntimeAdapter {
    fn spawn_local(&self, _future: LocalBoxFuture<'static, ()>) -> u64 {
        panic!("async runtime adapter not registered")
    }

    fn sleep(&self, _duration: std::time::Duration) -> LocalBoxFuture<'static, ()> {
        panic!("async runtime adapter not registered")
    }

    fn get_time(&self) -> std::time::Duration {
        panic!("async runtime adapter not registered")
    }
}
