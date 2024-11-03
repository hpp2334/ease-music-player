use std::{
    any::Any,
    cell::RefCell,
    fmt::Debug,
    rc::Rc,
    sync::{Arc, RwLock},
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

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        self._app.to_hosts.get::<C>()
    }

    pub fn flush_spawned(&self) {
        self._app.flush_spawned();
    }
}

unsafe impl Send for AppPod {}
unsafe impl Sync for AppPod {}

impl AppPod {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn set(&self, app: App) {
        app._app.check_same_thread();
        let mut w = self._app.write().expect("Failed to write App to AppPod");
        *w = Some(app._app.clone());
    }

    pub fn get(&self) -> App {
        let _app = self._app.read().expect("Failed to get App from AppPod");
        let _app = _app.clone().expect("App in AppPod is None");
        _app.check_same_thread();
        App { _app }
    }
}
