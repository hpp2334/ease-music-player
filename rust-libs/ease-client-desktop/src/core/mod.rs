use std::{
    sync::{atomic::AtomicBool, Arc, RwLock},
    thread::ThreadId,
    time::{Duration, SystemTime},
};

use ease_client::{
    build_client, to_host::connector::IConnectorHost, App, AppPod, EaseError, EaseResult,
    IPermissionService, IRouterService, IToastService, IViewStateService, WeakAppPod,
};
use ease_client_backend::{Backend, IPlayerDelegate};
use ease_client_shared::backends::{connector::IConnectorNotifier, music::MusicId, MessagePayload};
use futures::{channel::mpsc, future::BoxFuture, SinkExt};
use misty_lifecycle::{ILifecycleExternal, Runnable};

pub struct AppPodProxy(AppPod);

impl AppPodProxy {
    pub fn new(pod: AppPod) -> Self {
        Self(pod)
    }
    pub fn get(&self) -> App {
        self.0.get()
    }
}

impl gpui::Global for AppPodProxy {}

struct Player;

impl Player {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl IPlayerDelegate for Player {
    fn is_playing(&self) -> bool {
        false
    }

    fn resume(&self) {}

    fn pause(&self) {}

    fn stop(&self) {}

    fn seek(&self, arg: u64) {}

    fn set_music_url(&self, item: ease_client_backend::MusicToPlay) {}

    fn get_durations(&self) -> ease_client_shared::backends::player::PlayerDurations {
        ease_client_shared::backends::player::PlayerDurations::default()
    }

    fn request_total_duration(&self, id: MusicId, url: String) {}
}

pub struct BackendHost {
    _backend: Backend,
}

impl BackendHost {
    pub fn new(backend: Backend) -> Arc<Self> {
        Arc::new(Self { _backend: backend })
    }
}

impl IConnectorHost for BackendHost {
    fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        self._backend.connect(notifier)
    }

    fn disconnect(&self, handle: usize) {
        self._backend.disconnect(handle);
    }

    fn serve_music_url(&self, id: MusicId) -> String {
        self._backend.serve_music_url(id)
    }

    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>> {
        Box::pin(async move {
            self._backend
                .request(msg)
                .await
                .map_err(|e| EaseError::BackendChannelError(e.into()))
        })
    }

    fn storage_path(&self) -> String {
        self._backend.storage_path()
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

pub struct LifecycleExternal {
    foreground_sender: mpsc::Sender<Runnable>,
    background_executor: gpui::BackgroundExecutor,
}

impl ILifecycleExternal for LifecycleExternal {
    fn is_main_thread(&self) -> bool {
        self.background_executor.is_main_thread()
    }

    fn get_time(&self) -> Duration {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
    }

    // TODO: try to implement without spawn
    fn spawn_main_thread(&self, runnable: Runnable) {
        let mut tx = self.foreground_sender.clone();
        self.background_executor
            .spawn(async move {
                tx.send(runnable).await.unwrap();
            })
            .detach();
    }

    fn spawn(&self, runnable: Runnable) {
        self.background_executor
            .spawn(async move {
                runnable.run();
            })
            .detach();
    }

    fn spawn_sleep(&self, duration: Duration, runnable: Runnable) {
        let timer = self.background_executor.timer(duration);
        self.background_executor
            .spawn(async move {
                timer.await;
                runnable.run();
            })
            .detach();
    }
}

pub fn build_lifecycle(
    cx: &mut gpui::AppContext,
    foreground_sender: mpsc::Sender<Runnable>,
) -> Arc<LifecycleExternal> {
    Arc::new(LifecycleExternal {
        foreground_sender,
        background_executor: cx.background_executor().clone(),
    })
}

pub fn build_desktop_backend(lifecycle_external: Arc<LifecycleExternal>) -> Backend {
    Backend::new(lifecycle_external, Player::new())
}

pub fn build_desktop_client(
    lifecycle_external: Arc<LifecycleExternal>,
    backend: Backend,
    vs: impl IViewStateService,
) -> App {
    let app = build_client(
        BackendHost::new(backend),
        PermissionService::new(),
        RouterService::new(),
        ToastService::new(),
        Arc::new(vs),
        lifecycle_external,
    );
    app
}
