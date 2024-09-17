use std::{
    ops::Deref,
    sync::atomic::{AtomicBool, AtomicI32},
};

use crate::{
    async_task::{IAsyncTaskRuntimeAdapter, MistyAsyncTaskPools},
    resources::MistyResourceManager,
    schedule::ScheduleManager,
    services::MistyServiceManager,
    signals::SignalEmitter,
    states::MistyStateManager,
    views::ViewNotifier,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MistyClientId(i32);

const _: () = {
    static ALLOCATED: AtomicI32 = AtomicI32::new(1);
    impl MistyClientId {
        pub fn alloc() -> Self {
            let id = ALLOCATED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Self(id);
        }
    }
    impl Deref for MistyClientId {
        type Target = i32;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
};

pub(crate) struct MistyClientInner {
    pub id: MistyClientId,
    pub state_manager: MistyStateManager,
    pub view_manager: Box<dyn ViewNotifier + Send + Sync>,
    pub service_manager: MistyServiceManager,
    pub resource_manager: MistyResourceManager,
    pub async_task_pools: MistyAsyncTaskPools,
    pub async_task_runtime: Box<dyn IAsyncTaskRuntimeAdapter + Send + Sync>,
    pub schedule_manager: ScheduleManager,
    pub signal_emitter: SignalEmitter,
    pub destroyed: AtomicBool,
}

impl MistyClientInner {
    pub fn is_destroyed(&self) -> bool {
        self.destroyed.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn destroy(&self) {
        self.destroyed
            .swap(true, std::sync::atomic::Ordering::SeqCst);

        self.async_task_pools
            .reset(self.async_task_runtime.as_ref());
    }
}

impl Drop for MistyClientInner {
    fn drop(&mut self) {
        tracing::debug!("client {:?} is destroyed", self.id);
    }
}
