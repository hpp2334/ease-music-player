use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use futures::future::BoxFuture;
use misty_vm::{
    async_task::IAsyncTaskRuntimeAdapter,
    client::{MistyClientAccessor, SingletonMistyClientPod},
    controllers::{ControllerRet, MistyController},
    resources::{MistyResourceId, ResourceUpdateAction},
    services::MistyServiceManager,
    signals::MistySignal,
    states::MistyStateManager,
    views::MistyViewModelManager,
};

#[derive(Clone)]
pub struct TestAppContainer<R>
where
    R: Clone + Default,
{
    app: Arc<SingletonMistyClientPod<R>>,
    state: Arc<Mutex<R>>,
    resources: Arc<Mutex<HashMap<MistyResourceId, Vec<u8>>>>,
    apply_view: Arc<dyn Fn(R, &mut R) + Send + Sync + 'static>,
}
pub struct TestApp<R>
where
    R: Clone + Default,
{
    app: TestAppContainer<R>,
}

pub struct TokioAsyncTaskAdapter<R>
where
    R: Clone + Default,
{
    _app: TestAppContainer<R>,
    alloc: AtomicU64,
    store: Arc<Mutex<HashMap<u64, tokio::task::JoinHandle<()>>>>,
}

impl<R> IAsyncTaskRuntimeAdapter for TokioAsyncTaskAdapter<R>
where
    R: Clone + Default + Send + Sync,
{
    fn spawn(&self, future: BoxFuture<'static, ()>) -> u64 {
        let handle = tokio::spawn(future);
        let id = self.alloc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        {
            let mut w = self.store.lock().unwrap();
            w.insert(id, handle);
        }
        id
    }

    fn spawn_local(&self, future: futures::prelude::future::LocalBoxFuture<'static, ()>) -> u64 {
        panic!("spawn_local is not implemented yet");
    }

    fn try_abort(&self, task_id: u64) {
        let handle = {
            let mut w = self.store.lock().unwrap();
            w.remove(&task_id)
        };
        if let Some(handle) = handle {
            handle.abort();
        }
    }
}

impl<R> TestAppContainer<R>
where
    R: Default + Clone + Send + Sync + 'static,
{
    pub fn new(apply_view: impl Fn(R, &mut R) + Send + Sync + 'static) -> Self {
        Self {
            app: Arc::new(SingletonMistyClientPod::new()),
            state: Default::default(),
            resources: Default::default(),
            apply_view: Arc::new(apply_view),
        }
    }

    pub fn call_controller<Controller, Arg, E>(&self, controller: Controller, arg: Arg)
    where
        Controller: MistyController<Arg, E>,
        E: std::fmt::Debug,
    {
        let ret = self.app.call_controller(controller, arg).unwrap();
        self.apply(ret);
    }

    pub fn flush_schedules(&self) {
        let ret = self.app.flush_scheduled_tasks().unwrap();
        self.apply(ret);
    }

    pub fn get_resource(&self, id: MistyResourceId) -> Option<Vec<u8>> {
        let w = self.resources.lock().unwrap();
        w.get(&id).map(|r| r.clone())
    }

    pub fn accessor(&self) -> MistyClientAccessor {
        self.app.accessor()
    }

    fn apply(&self, ret: ControllerRet<R>) {
        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(changed_view) = ret.changed_view {
                (self.apply_view)(changed_view, &mut state_guard);
            }
        }

        {
            let mut w = self.resources.lock().unwrap();

            for resource in ret.changed_resources.into_iter() {
                match resource {
                    ResourceUpdateAction::Insert(id, buf) => {
                        w.insert(id, buf);
                    }
                    ResourceUpdateAction::Remove(id) => {
                        w.remove(&id);
                    }
                }
            }
        }
    }
}
impl<R> TestApp<R>
where
    R: Default + Clone + Send + Sync + 'static,
{
    pub fn new(
        view_manager: MistyViewModelManager<R>,
        service_manager: MistyServiceManager,
        state_manager: MistyStateManager,
        app_container: TestAppContainer<R>,
    ) -> Self {
        let adapter = TokioAsyncTaskAdapter {
            _app: app_container.clone(),
            alloc: Default::default(),
            store: Default::default(),
        };

        app_container
            .app
            .create(view_manager, state_manager, service_manager, adapter);

        let cloned = app_container.clone();
        app_container.app.on_signal(move |signal| match signal {
            MistySignal::Schedule => {
                cloned.flush_schedules();
            }
        });

        Self { app: app_container }
    }

    pub fn app(&self) -> TestAppContainer<R> {
        self.app.clone()
    }

    pub fn state(&self) -> R {
        self.app().state.lock().unwrap().clone()
    }
}
