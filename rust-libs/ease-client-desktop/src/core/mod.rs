use std::{
    sync::{atomic::AtomicBool, Arc, RwLock},
    thread::ThreadId,
    time::{Duration, SystemTime},
};

use ease_client::{
    build_client, to_host::connector::IConnectorHost, App, AppPod, EaseError, EaseResult,
    IPermissionService, IRouterService, IToastService, IViewStateService, WeakAppPod,
};
use ease_client_backend::Backend;
use ease_client_shared::backends::{connector::IConnectorNotifier, music::MusicId, MessagePayload};
use futures::future::BoxFuture;

pub struct AppPodProxy(pub AppPod);

impl gpui::Global for AppPodProxy {}

pub struct BackendHost {
    _backend: RwLock<Option<Arc<Backend>>>,
}

impl BackendHost {
    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            _backend: Default::default(),
        });
        let cloned = this.clone();

        cloned
    }

    pub fn has_backend(&self) -> bool {
        self._backend.read().unwrap().is_some()
    }

    pub fn set_backend(&self, backend: Arc<Backend>) {
        assert!(!self.has_backend());
        let mut w = self._backend.write().unwrap();
        *w = Some(backend);
    }

    pub fn reset_backend(&self) {
        assert!(self.has_backend());
        let mut w = self._backend.write().unwrap();
        *w = None;
    }

    pub fn backend(&self) -> Arc<Backend> {
        let backend = self._backend.read().unwrap();
        backend.as_ref().unwrap().clone()
    }
    pub fn try_backend(&self) -> Option<Arc<Backend>> {
        let backend = self._backend.read().unwrap();
        backend.clone()
    }
}

impl IConnectorHost for BackendHost {
    fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        self.backend().connect(notifier)
    }

    fn disconnect(&self, handle: usize) {
        self.backend().disconnect(handle);
    }

    fn serve_music_url(&self, id: MusicId) -> String {
        self.backend().serve_music_url(id)
    }

    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>> {
        Box::pin(async move {
            self.backend()
                .request(msg)
                .await
                .map_err(|e| EaseError::BackendChannelError(e.into()))
        })
    }

    fn storage_path(&self) -> String {
        self.backend().storage_path()
    }
}

struct PermissionService;
impl PermissionService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}
impl IPermissionService for PermissionService {
    fn open_url(&self, url: String) {
        todo!()
    }

    fn have_storage_permission(&self) -> bool {
        return true;
    }

    fn request_storage_permission(&self) {}
}

struct RouterService;
impl RouterService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}
impl IRouterService for RouterService {
    fn naviagate(&self, key: ease_client::RoutesKey) {}

    fn pop(&self) {}
}

struct ToastService;
impl ToastService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}
impl IToastService for ToastService {
    fn error(&self, msg: String) {}
}

struct ViewStateService;
impl IViewStateService for ViewStateService {
    fn handle_notify(&self, v: ease_client::RootViewModelState) {}
}

#[derive(Clone)]
pub struct GpuiContextWrapper {
    thread_id: ThreadId,
    // gpui_cx: gpui::AsyncAppContext,
    background_executor: gpui::BackgroundExecutor,
}

impl GpuiContextWrapper {
    pub fn new(cx: &mut gpui::AppContext) -> Self {
        let background_executor = cx.background_executor().clone();

        Self {
            thread_id: std::thread::current().id(),
            // gpui_cx: cx.to_async(),
            background_executor,
        }
    }

    fn context(&self) -> &gpui::AsyncAppContext {
        // assert!(self.is_main_thread());
        // &self.gpui_cx
        todo!()
    }

    fn background_executor(&self) -> &gpui::BackgroundExecutor {
        &self.background_executor
    }

    fn is_main_thread(&self) -> bool {
        self.thread_id == std::thread::current().id()
    }
}

struct AsyncRuntimeAdapter {
    wrapper: GpuiContextWrapper,
    handle_spawned_next_tick: Arc<AtomicBool>,
    pod: WeakAppPod,
}

impl IAsyncDispatcher for AsyncRuntimeAdapter {
    fn is_main_thread(&self) -> bool {
        self.wrapper.is_main_thread()
    }

    fn on_spawn_locals(&self) {
        // let state = self.handle_spawned_next_tick.clone();

        // let has_handle = state.swap(true, std::sync::atomic::Ordering::SeqCst);

        // if !has_handle {
        //     let executor = self.wrapper.foreground_executor().clone();
        //     let background_executor = self.wrapper.background_executor().clone();
        //     let pod = self.pod.clone();
        //     executor.clone().spawn(async move {
        //         state.store(false, std::sync::atomic::Ordering::SeqCst);

        //         background_executor.timer(Duration::ZERO).await;
        //         if let Some(app) = pod.get() {
        //             app.flush_spawned();
        //         }
        //     });
        // }
    }

    fn get_time(&self) -> Duration {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        let background_executor = self.wrapper.background_executor().clone();
        Box::pin(async move { background_executor.timer(duration).await })
    }
}

pub fn build_desktop_client(cx: &mut gpui::AppContext, vs: impl IViewStateService) -> AppPodProxy {
    let pod = AppPod::new();
    let app = build_client(
        BackendHost::new(),
        PermissionService::new(),
        RouterService::new(),
        ToastService::new(),
        Arc::new(vs),
        Arc::new(AsyncRuntimeAdapter {
            wrapper: GpuiContextWrapper::new(cx),
            handle_spawned_next_tick: Default::default(),
            pod: pod.weak(),
        }),
    );
    pod.set(app);

    AppPodProxy(pod)
}
