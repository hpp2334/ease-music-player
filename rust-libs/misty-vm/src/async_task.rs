use std::{
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    sync::{atomic::AtomicU64, Arc, RwLock, Weak},
};

use futures::future::{BoxFuture, LocalBoxFuture};

use crate::{
    client::{
        AsMistyClientHandle, AsReadonlyMistyClientHandle, MistyClientAccessor, MistyClientHandle,
        MistyClientInner, MistyReadonlyClientHandle,
    },
    utils::PhantomUnsync,
};

pub trait IAsyncTaskRuntimeAdapter {
    fn spawn(&self, future: BoxFuture<'static, ()>) -> u64;
    fn spawn_local(&self, future: LocalBoxFuture<'static, ()>) -> u64;
    fn try_abort(&self, task_id: u64);
}

pub struct MistyAsyncTaskContext {
    pub(crate) inner: Weak<MistyClientInner>,
}

pub struct MistyClientAsyncHandleGuard {
    inner: Option<Arc<MistyClientInner>>,
    _unsync_marker: PhantomUnsync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MistyAsyncTask {
    id: u64,
    host_task_id: u64,
}

fn alloc_task_id() -> u64 {
    static ALLOCATED: AtomicU64 = AtomicU64::new(1);
    let id = ALLOCATED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    id
}

#[derive(Debug, Default)]
struct InternalMistyAsyncTaskPool {
    async_tasks: HashMap<u64, MistyAsyncTask>,
}

#[derive(Debug, Clone, Default)]
struct BoxedMistyAsyncTaskPool {
    pool: Arc<RwLock<InternalMistyAsyncTaskPool>>,
}

#[derive(Debug)]
struct MistyAsyncTaskPool<T> {
    pool: Arc<RwLock<InternalMistyAsyncTaskPool>>,
    _marker: PhantomData<T>,
}

type InternalMistyAsyncTaskPools = HashMap<TypeId, BoxedMistyAsyncTaskPool>;

#[derive(Debug)]
pub(crate) struct MistyAsyncTaskPools {
    pools: Arc<RwLock<InternalMistyAsyncTaskPools>>,
}

struct MistyAsyncTaskPoolSpawnCleanupGuard<T>
where
    T: MistyAsyncTaskTrait,
{
    task_id: u64,
    marker: PhantomData<T>,
    pools: Weak<RwLock<InternalMistyAsyncTaskPools>>,
}

impl<T> Drop for MistyAsyncTaskPoolSpawnCleanupGuard<T>
where
    T: MistyAsyncTaskTrait,
{
    fn drop(&mut self) {
        if let Some(pools) = self.pools.upgrade() {
            let tid = std::any::TypeId::of::<T>();
            let pool = pools.write().unwrap();
            let pool = pool.get(&tid);
            if let Some(pool) = pool {
                let mut pool = pool.pool.write().unwrap();
                pool.async_tasks.remove(&self.task_id);
            }
        }
    }
}

impl MistyAsyncTaskPools {
    pub fn new() -> Self {
        Self {
            pools: Default::default(),
        }
    }

    fn get<T: 'static>(&self) -> MistyAsyncTaskPool<T> {
        let pool = {
            let mut pools = self.pools.write().unwrap();
            let pool = pools
                .entry(std::any::TypeId::of::<T>())
                .or_default()
                .clone();
            pool
        };

        MistyAsyncTaskPool {
            pool: pool.pool.clone(),
            _marker: Default::default(),
        }
    }

    pub(crate) fn reset(&self, rt: &dyn IAsyncTaskRuntimeAdapter) {
        let mut pools = self.pools.write().unwrap();

        for (_, pool) in pools.iter() {
            let mut pool = pool.pool.write().unwrap();
            for (_, task) in pool.async_tasks.iter() {
                rt.try_abort(task.host_task_id);
            }
            pool.async_tasks.clear();
        }
        pools.clear();
    }
}

impl<T> MistyAsyncTaskPool<T>
where
    T: MistyAsyncTaskTrait,
{
    pub fn spawn<R, E>(
        &self,
        handle: MistyReadonlyClientHandle,
        future_fn: impl (FnOnce(MistyAsyncTaskContext) -> R) + Send + 'static,
    ) where
        R: std::future::Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Display,
    {
        let inner = handle.inner.clone();
        let cloned_inner = inner.clone();
        let task_id = alloc_task_id();

        let host_task_id = inner.async_task_runtime.spawn(Box::pin(async move {
            let inner = cloned_inner;
            let _guard = MistyAsyncTaskPoolSpawnCleanupGuard::<T> {
                task_id,
                marker: Default::default(),
                pools: Arc::downgrade(&inner.async_task_pools.pools),
            };

            let ctx = MistyAsyncTaskContext::new(Arc::downgrade(&inner));
            let res = future_fn(ctx).await;
            if res.is_err() {
                let e = res.unwrap_err();
                tracing::error!("spawn error: {}", e);
            }
        }));

        let task = MistyAsyncTask {
            id: task_id,
            host_task_id,
        };
        {
            let mut pool = self.pool.write().unwrap();
            pool.async_tasks.insert(task_id, task);
        }
    }

    pub fn spawn_local<R, E>(
        &self,
        handle: MistyReadonlyClientHandle,
        future_fn: impl (FnOnce(MistyAsyncTaskContext) -> R) + 'static,
    ) where
        R: std::future::Future<Output = Result<(), E>> + 'static,
        E: std::fmt::Display,
    {
        let inner = handle.inner.clone();
        let cloned_inner = inner.clone();
        let task_id = alloc_task_id();

        let host_task_id = inner.async_task_runtime.spawn_local(Box::pin(async move {
            let inner = cloned_inner;
            let _guard = MistyAsyncTaskPoolSpawnCleanupGuard::<T> {
                task_id,
                marker: Default::default(),
                pools: Arc::downgrade(&inner.async_task_pools.pools),
            };

            let ctx = MistyAsyncTaskContext::new(Arc::downgrade(&inner));
            let res = future_fn(ctx).await;
            if res.is_err() {
                let e = res.unwrap_err();
                tracing::error!("spawn error: {}", e);
            }
        }));

        let task = MistyAsyncTask {
            id: task_id,
            host_task_id,
        };
        {
            let mut pool = self.pool.write().unwrap();
            pool.async_tasks.insert(task_id, task);
        }
    }

    pub fn cancel_all(&self, rt: &dyn IAsyncTaskRuntimeAdapter) {
        let mut pool = self.pool.write().unwrap();

        for (_, task) in pool.async_tasks.iter() {
            rt.try_abort(task.host_task_id);
        }
        pool.async_tasks.clear();
    }
}

impl MistyClientAsyncHandleGuard {
    pub fn handle(&self) -> MistyReadonlyClientHandle {
        // SAFETY: spawned task will be aborted when client destroyed
        let inner = self.inner.as_ref().unwrap();
        MistyReadonlyClientHandle { inner }
    }
}

impl MistyAsyncTaskContext {
    fn new(inner: Weak<MistyClientInner>) -> Self {
        Self { inner }
    }

    pub fn handle(&self) -> MistyClientAsyncHandleGuard {
        let inner = self.inner.upgrade();
        MistyClientAsyncHandleGuard {
            inner,
            _unsync_marker: Default::default(),
        }
    }

    pub fn accessor(&self) -> MistyClientAccessor {
        MistyClientAccessor {
            inner: self.inner.clone(),
        }
    }

    pub fn schedule<E>(
        &self,
        handler: impl FnOnce(MistyClientHandle) -> Result<(), E> + Send + Sync + 'static,
    ) where
        E: std::fmt::Display,
    {
        let client = self.inner.upgrade();
        if client.is_none() {
            return;
        }
        let inner = client.unwrap();

        if inner.is_destroyed() {
            tracing::warn!("schedule but client is destroyed");
            return;
        }
        inner
            .schedule_manager
            .enqueue(&inner.signal_emitter, handler);
    }
}

pub trait MistyAsyncTaskTrait: Sized + Send + Sync + 'static {
    fn spawn_once<'a, T, E>(
        cx: impl AsMistyClientHandle<'a>,
        future_fn: impl (FnOnce(MistyAsyncTaskContext) -> T) + Send + Sync + 'static,
    ) where
        T: std::future::Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Display,
    {
        let inner = cx.handle().inner;
        let pool = inner.async_task_pools.get::<Self>();
        pool.cancel_all(inner.async_task_runtime.as_ref());
        pool.spawn(cx.readonly_handle().clone(), future_fn);
    }

    fn spawn<'a, T, E>(
        cx: impl AsMistyClientHandle<'a>,
        future_fn: impl (FnOnce(MistyAsyncTaskContext) -> T) + Send + Sync + 'static,
    ) where
        T: std::future::Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Display,
    {
        let pool = cx.handle().inner.async_task_pools.get::<Self>();
        pool.spawn(cx.readonly_handle(), future_fn);
    }

    fn spawn_local_once<'a, T, E>(
        cx: impl AsMistyClientHandle<'a>,
        future_fn: impl (FnOnce(MistyAsyncTaskContext) -> T) + 'static,
    ) where
        T: std::future::Future<Output = Result<(), E>> + 'static,
        E: std::fmt::Display,
    {
        let inner = cx.handle().inner;
        let pool = inner.async_task_pools.get::<Self>();
        pool.cancel_all(inner.async_task_runtime.as_ref());
        pool.spawn_local(cx.readonly_handle().clone(), future_fn);
    }

    fn spawn_local<'a, T, E>(
        cx: impl AsMistyClientHandle<'a>,
        future_fn: impl (FnOnce(MistyAsyncTaskContext) -> T) + 'static,
    ) where
        T: std::future::Future<Output = Result<(), E>> + 'static,
        E: std::fmt::Display,
    {
        let pool = cx.handle().inner.async_task_pools.get::<Self>();
        pool.spawn_local(cx.readonly_handle(), future_fn);
    }

    fn cancel_all<'a>(cx: impl AsMistyClientHandle<'a>) {
        let inner = cx.handle().inner;
        let pool = inner.async_task_pools.get::<Self>();
        pool.cancel_all(inner.async_task_runtime.as_ref());
    }
}
