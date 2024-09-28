use std::sync::Arc;

use crate::{
    async_task::AsyncTasks,
    models::Models,
    to_host::{ToHosts, ToHostsBuilder},
    view_models::BoxedViewModels,
};

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
        self._app.read_model()
    }

    pub fn emit<Event>(&self, evt: Event)
    where
        Event: 'static,
    {
        self._app.emit(evt);
    }
}
