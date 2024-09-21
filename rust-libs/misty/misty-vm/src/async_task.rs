use std::{
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    sync::{atomic::AtomicU64, Arc, RwLock, Weak},
};

use futures::future::{BoxFuture, LocalBoxFuture};

pub trait IAsyncRuntimeAdapter: 'static {
    fn spawn_local(&self, future: LocalBoxFuture<'static, ()>) -> u64;
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
}

pub(crate) struct DefaultAsyncRuntimeAdapter;

impl IAsyncRuntimeAdapter for DefaultAsyncRuntimeAdapter {
    fn spawn_local(&self, _future: LocalBoxFuture<'static, ()>) -> u64 {
        panic!("async runtime adapter not registered")
    }
}
