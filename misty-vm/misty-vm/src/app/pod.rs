use std::{cell::RefCell, sync::Arc};

use crate::IToHost;

use super::{builder::AppBuilder, internal::AppInternal};

pub struct App {
    pub(crate) _app: Arc<AppInternal>,
}

pub struct AppPod {
    _app: RefCell<Option<Arc<AppInternal>>>,
}

impl App {
    pub fn builder<Event, E>() -> AppBuilder<Event, E>
    where
        Event: 'static,
        E: 'static,
    {
        AppBuilder::new()
    }

    pub fn model<T>(&self) -> std::cell::Ref<'_, T>
    where
        T: 'static,
    {
        self._app.model_get()
    }

    pub fn emit<Event>(&self, evt: Event)
    where
        Event: 'static,
    {
        self._app.emit(evt);
    }

    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        self._app.to_host::<C>()
    }
}

unsafe impl Send for AppPod {}
unsafe impl Sync for AppPod {}
impl AppPod {
    pub fn new() -> Self {
        Self {
            _app: RefCell::new(None)
        }
    }
    
    pub fn set(&self, app: App) {
        self.check_same_thread();

        let mut w = self._app.borrow_mut();
        *w = Some(app._app.clone());
    }

    pub fn get(&self) -> App {
        self.check_same_thread();

        let _app = self._app.borrow().clone().unwrap();
        App {
            _app
        }
    }
    
    fn check_same_thread(&self) {
        let app = self._app.borrow();
        if let Some(app) = app.as_ref() {
            let thread_id = std::thread::current().id();
            if app.thread_id != thread_id {
                panic!("cannot operate app in other thread")
            }
        }
    }
}
