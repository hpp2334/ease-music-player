use std::{
    cell::RefCell, rc::Rc, sync::{atomic::AtomicBool, Arc, RwLock}, thread::ThreadId, time::{Duration, SystemTime}
};

use ease_client::{
    build_client, to_host::connector::IConnectorHost, Action, AndroidRoutesKey, App, AppPod, DesktopRoutesKey, EaseError, EaseResult, IPermissionService, IRouterService, IToastService, IViewStateService, ViewAction, WeakAppPod, WidgetAction
};
use ease_client_backend::{Backend, IPlayerDelegate};
use ease_client_shared::backends::{connector::IConnectorNotifier, music::MusicId, MessagePayload};
use futures::{channel::mpsc, future::BoxFuture, SinkExt};
use misty_lifecycle::{ILifecycleExternal, Runnable};

use super::view_state::{GpuiViewStateService, RouteStack, ViewStates};

#[derive(Clone)]
pub struct AppBridge {
    pod: Rc<AppPod>,
    vs: Rc<RefCell<Option<ease_client::RootViewModelState>>>,
    routes: Rc<RefCell<RouteStack>>,
    gpui_vs: ViewStates,
}

impl AppBridge {
    pub fn dispatch<C>(&self, cx: &mut C, action: Action)
    where C: gpui::Context {
        self.pod.get().emit(action);
        self.flush(cx);
    }
    pub fn dispatch_widget<C>(&self, cx: &mut C, action: WidgetAction)
    where C: gpui::Context {
        self.pod.get().emit(Action::View(ViewAction::Widget(action)));
        self.flush(cx);
    }

    pub fn flush<C>(&self, cx: &mut C)
    where C: gpui::Context {
        {
            let v = self.vs.borrow_mut().take();
            if let Some(v) = v {
                let u = v.playlist_list.clone();
                if u.is_some() {
                    let state = self.gpui_vs.playlist_list.clone();
                    state.update(cx, |v, _| {
                        *v = u.unwrap();
                    });
                }
            }
        }
        {
            let mut v = self.routes.borrow_mut();
            if v.dirty {
                v.dirty = false;
                self.gpui_vs.route_stack.update(cx, |dst, _| {
                    *dst = v.clone();
                });
            }
        }
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
    routes: Rc<RefCell<RouteStack>>,
}
impl RouterService {
    pub fn new(routes: Rc<RefCell<RouteStack>>) -> Arc<Self> {
        Arc::new(Self {
            routes,
        })
    }
}
impl IRouterService for RouterService {
    fn navigate(&self, key: AndroidRoutesKey) {}
    fn navigate_desktop(&self, key: DesktopRoutesKey) {
        tracing::info!("{:?}", key);

        let routes = self.routes.clone();
        let mut routes = routes.borrow_mut();
        routes.dirty = true;
        routes.routes.push(key);
    }
    fn pop(&self) {
        let routes = self.routes.clone();
        let mut routes = routes.borrow_mut();
        routes.dirty = true;
        routes.routes.pop();
    }
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
    vs: ViewStates,
) -> AppBridge {
    let rvs: Rc<RefCell<Option<ease_client::RootViewModelState>>> = Default::default();
    let routes: Rc<RefCell<RouteStack>> = Default::default();

    let app = build_client(
        BackendHost::new(backend),
        PermissionService::new(),
        RouterService::new(routes.clone()),
        ToastService::new(),
        Arc::new(GpuiViewStateService::new(rvs.clone())),
        lifecycle_external,
    );
    
    let pod = AppPod::new();
    pod.set(app);
    
    AppBridge {
        pod: Rc::new(pod),
        vs: rvs,
        routes,
        gpui_vs: vs,
    }
}
