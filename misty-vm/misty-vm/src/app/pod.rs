use std::sync::Arc;

use crate::IToHost;

use super::{builder::AppBuilder, internal::AppInternal};

pub struct App {
    pub(crate) _app: Arc<AppInternal>,
}

impl App {
    pub fn builder<Event, E>() -> AppBuilder<Event, E>
    where
        Event: 'static,
        E: 'static,
    {
        AppBuilder::new()
    }

    pub fn start(&self) {
        self._app.start();
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
