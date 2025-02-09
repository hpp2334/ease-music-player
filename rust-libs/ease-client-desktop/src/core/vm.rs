use std::{
    cell::RefCell,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc, RwLock},
    thread::ThreadId,
    time::{Duration, SystemTime},
};

use crate::utils::dynamic_lifetime::SharedDynamicLifetime;

use super::routes::Router;
use ease_client::{
    build_client, to_host::connector::IConnectorHost, Action, AndroidRoutesKey, App, AppPod,
    DesktopRoutesKey, EaseError, EaseResult, IPermissionService, IRouterService, IToastService,
    IViewStateService, ViewAction, WeakAppPod, WidgetAction,
};
use ease_client_backend::{Backend, IPlayerDelegate};
use ease_client_shared::backends::{connector::IConnectorNotifier, music::MusicId, MessagePayload};
use futures::{channel::mpsc, future::BoxFuture, SinkExt};
use gpui::{AppContext, Entity};
use misty_lifecycle::{ILifecycleExternal, Runnable};
use std::fmt::Debug;

use super::view_state::{GpuiViewStateService, ViewStates};

#[derive(Clone)]
pub struct AppBridge {
    pod: Rc<AppPod>,
    dyn_app: SharedDynamicLifetime<gpui::App>,
}

impl AppBridge {
    pub fn dispatch(&self, cx: &mut gpui::App, action: Action) {
        let _guard = self.dyn_app.wrap(cx);
        self.pod.get().emit(action);
    }
    pub fn dispatch_widget(&self, cx: &mut gpui::App, action: WidgetAction) {
        let _guard = self.dyn_app.wrap(cx);
        self.pod
            .get()
            .emit(Action::View(ViewAction::Widget(action)));
    }

    pub fn flush_with(&self, cx: &mut gpui::App, block: impl FnOnce()) {
        let _guard = self.dyn_app.wrap(cx);
        block();
    }
}

impl gpui::Global for AppBridge {}

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

struct RouterService {
    router: Entity<Router>,
    dyn_app: SharedDynamicLifetime<gpui::App>,
}
impl RouterService {
    pub fn new(router: Entity<Router>, dyn_app: SharedDynamicLifetime<gpui::App>) -> Arc<Self> {
        Arc::new(Self { router, dyn_app })
    }
}
impl IRouterService for RouterService {
    fn navigate(&self, key: AndroidRoutesKey) {}
    fn navigate_desktop(&self, key: DesktopRoutesKey) {
        let mut cx = self.dyn_app.get();
        let cx = cx.get();
        self.router.update(cx, |r, _| {
            r.push(key);
        });
    }
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
    cx: &mut gpui::App,
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
    cx: &mut gpui::App,
    lifecycle_external: Arc<LifecycleExternal>,
    backend: Backend,
) -> (AppBridge, ViewStates) {
    let vs = ViewStates::new(cx);
    let dyn_app: SharedDynamicLifetime<gpui::App> = Default::default();

    let app = build_client(
        BackendHost::new(backend),
        PermissionService::new(),
        RouterService::new(vs.router.clone(), dyn_app.clone()),
        ToastService::new(),
        Arc::new(GpuiViewStateService::new(vs.clone(), dyn_app.clone())),
        lifecycle_external,
    );

    let pod = AppPod::new();
    pod.set(app);

    (
        AppBridge {
            pod: Rc::new(pod),
            dyn_app,
        },
        vs,
    )
}
