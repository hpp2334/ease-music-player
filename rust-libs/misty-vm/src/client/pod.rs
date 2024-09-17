use std::{
    any::Any,
    convert::Infallible,
    marker::PhantomData,
    sync::{atomic::AtomicBool, Arc, RwLock},
};

use once_cell::sync::Lazy;

use crate::{
    async_task::{IAsyncTaskRuntimeAdapter, MistyAsyncTaskPools},
    controllers::{call_controller, ControllerRet, MistyController},
    resources::MistyResourceManager,
    schedule::{controller_flush_scheduled_tasks, ScheduleManager},
    services::MistyServiceManager,
    signals::{MistySignal, SignalEmitter},
    states::MistyStateManager,
    views::MistyViewModelManager,
};

use super::{MistyClientAccessor, MistyClientId, MistyClientInner};

struct MistyClient<R> {
    inner: Arc<MistyClientInner>,
    _marker: PhantomData<R>,
}

impl<R> MistyClient<R>
where
    R: Any + Default + Send + Sync + 'static,
{
    fn new(
        view_manager: MistyViewModelManager<R>,
        state_manager: MistyStateManager,
        service_manager: MistyServiceManager,
        async_task_runtime: impl IAsyncTaskRuntimeAdapter + Send + Sync + 'static,
    ) -> Self {
        let inner = Arc::new(MistyClientInner {
            id: MistyClientId::alloc(),
            state_manager,
            view_manager: Box::new(view_manager),
            service_manager,
            resource_manager: MistyResourceManager::new(),
            async_task_pools: MistyAsyncTaskPools::new(),
            async_task_runtime: Box::new(async_task_runtime),
            schedule_manager: ScheduleManager::new(),
            signal_emitter: SignalEmitter::new(),
            destroyed: AtomicBool::new(false),
        });

        Self {
            inner,
            _marker: Default::default(),
        }
    }
}

pub struct SingletonMistyClientPod<R> {
    client: Lazy<RwLock<Option<MistyClient<R>>>>,
}

impl<R> SingletonMistyClientPod<R>
where
    R: Any + Default + Send + Sync + 'static,
{
    pub const fn new() -> Self {
        Self {
            client: Lazy::new(|| Default::default()),
        }
    }

    pub fn reset(&self) {
        let _ = tracing::span!(tracing::Level::INFO, "SingletonMistyClientPod.reset").enter();
        let pod = self.client.write().unwrap().take();
        if let Some(client) = pod {
            client.inner.destroy();
        }
    }

    pub fn destroy(&self) {
        self.inner().destroy();
    }

    pub fn create(
        &self,
        view_manager: MistyViewModelManager<R>,
        state_manager: MistyStateManager,
        service_manager: MistyServiceManager,
        async_task_runtime: impl IAsyncTaskRuntimeAdapter + Send + Sync + 'static,
    ) {
        let _ = tracing::span!(tracing::Level::INFO, "SingletonMistyClientPod.set").enter();

        let mut pod = self.client.write().unwrap();
        if pod.is_some() {
            panic!("client is already in pod");
        }
        *pod = Some(MistyClient::new(
            view_manager,
            state_manager,
            service_manager,
            async_task_runtime,
        ));
    }

    pub fn call_controller<Controller, Arg, E>(
        &self,
        controller: Controller,
        arg: Arg,
    ) -> Result<ControllerRet<R>, E>
    where
        Controller: MistyController<Arg, E>,
    {
        let inner = {
            let pod = self.client.read().unwrap();
            if let Some(client) = pod.as_ref() {
                client.inner.clone()
            } else {
                panic!(
                    "client not in singleton pod. controller is {}",
                    std::any::type_name::<Controller>()
                );
            }
        };
        call_controller(&inner, controller, arg)
    }

    pub fn on_signal(&self, f: impl Fn(MistySignal) + Send + Sync + 'static) {
        let inner = self.inner();
        inner.signal_emitter.set(f);
    }

    pub fn flush_scheduled_tasks(&self) -> Result<ControllerRet<R>, Infallible> {
        self.call_controller(controller_flush_scheduled_tasks, ())
    }

    pub fn accessor(&self) -> MistyClientAccessor {
        MistyClientAccessor {
            inner: Arc::downgrade(&self.inner()),
        }
    }

    fn inner(&self) -> Arc<MistyClientInner> {
        let pod = self.client.read().unwrap();
        if let Some(client) = pod.as_ref() {
            client.inner.clone()
        } else {
            panic!("client not in singleton pod.");
        }
    }
}
