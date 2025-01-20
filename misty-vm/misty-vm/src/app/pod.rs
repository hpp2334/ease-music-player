use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc, RwLock, Weak},
};

use crate::IToHost;

use super::{builder::AppBuilder, internal::AppInternal};

#[derive(Clone)]
pub struct App {
    pub(crate) _app: Arc<AppInternal>,
}

#[derive(Clone, Default)]
pub struct AppPod {
    _app: Arc<RwLock<Option<Arc<AppInternal>>>>,
}

unsafe impl Send for AppPod {}
unsafe impl Sync for AppPod {}

#[derive(Clone)]
pub struct WeakAppPod {
    _app: Weak<RwLock<Option<Arc<AppInternal>>>>,
}

unsafe impl Send for WeakAppPod {}
unsafe impl Sync for WeakAppPod {}

#[derive(Default)]
pub struct AppPods {
    _apps: Arc<RwLock<HashMap<u64, Arc<AppInternal>>>>,
    _alloc: AtomicU64,
}
unsafe impl Send for AppPods {}
unsafe impl Sync for AppPods {}

impl App {
    pub fn builder<Event>() -> AppBuilder<Event>
    where
        Event: 'static,
    {
        AppBuilder::new()
    }

    pub fn model<T>(&self) -> std::cell::Ref<'_, T>
    where
        T: 'static,
    {
        self._app.models.read()
    }

    pub fn emit<Event>(&self, evt: Event)
    where
        Event: Any + Debug + 'static,
    {
        self._app.emit(evt);
    }

    pub fn to_host<C>(&self) -> Rc<C>
    where
        C: IToHost,
    {
        self._app.to_hosts.get::<C>()
    }
}

impl Drop for AppPod {
    fn drop(&mut self) {
        tracing::info!("drop AppPod")
    }
}

impl AppPod {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn reset(&self) {
        self._app.write().unwrap().take();
    }

    pub fn set(&self, app: App) {
        app._app.local.check_same_thread();
        let mut w = self._app.write().expect("Failed to write App to AppPod");
        *w = Some(app._app.clone());
    }

    pub fn get(&self) -> App {
        let _app = self._app.read().expect("Failed to get App from AppPod");
        let _app = _app.clone().expect("App in AppPod is None");
        _app.local.check_same_thread();
        App { _app }
    }

    pub fn weak(&self) -> WeakAppPod {
        WeakAppPod {
            _app: Arc::downgrade(&self._app),
        }
    }
}

impl WeakAppPod {
    pub fn get(&self) -> Option<App> {
        if let Some(app) = self._app.upgrade() {
            let _app = app.read().expect("Failed to get App from AppPod");
            let _app = _app.clone();
            if let Some(_app) = _app {
                Some(App { _app })
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl AppPods {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn preallocate(&self) -> u64 {
        let id = self
            ._alloc
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        id
    }

    pub fn allocate(&self, id: u64, app: App) {
        app._app.local.check_same_thread();
        let mut apps = self._apps.write().unwrap();
        apps.insert(id, app._app.clone());
    }

    pub fn try_get(&self, handle: u64) -> Option<App> {
        let app = {
            let apps = self._apps.read().unwrap();
            apps.get(&handle).map(|v| v.clone()).clone()
        };
        let _app = app;
        if let Some(_app) = _app {
            _app.local.check_same_thread();
            Some(App { _app })
        } else {
            None
        }
    }

    pub fn get(&self, handle: u64) -> App {
        self.try_get(handle).unwrap()
    }

    pub fn take(&self, handle: u64) -> App {
        let app = {
            let mut apps = self._apps.write().unwrap();
            apps.remove(&handle).map(|v| v.clone()).clone()
        };
        let _app = app.unwrap();
        _app.local.check_same_thread();
        App { _app }
    }
}
