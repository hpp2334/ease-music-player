use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
};

use crate::{
    async_task::{AsyncTasks, IAsyncRuntimeAdapter}, models::Models, to_host::ToHosts, view_models::BoxedViewModels, IToHost, Model, ViewModelContext
};

pub(crate) struct AppInternal {
    pub models: Models,
    pub view_models: Box<dyn BoxedViewModels>,
    pub to_hosts: ToHosts,
    pub async_tasks: AsyncTasks,
}

impl AppInternal {
    pub fn emit<Event>(self: &Arc<AppInternal>, evt: Event)
    where
        Event: 'static,
    {
        self.view_models.handle_event(self, Box::new(evt));
    }

    pub fn start(self: &Arc<AppInternal>) {
        let cx = ViewModelContext::new(self.clone());
        self.view_models.handle_start(&cx);
    }

    pub fn read_model<T>(&self) -> std::cell::Ref<'_, T>
    where
        T: 'static,
    {
        self.models.read()
    }

    pub fn update_model<T>(self: &Arc<AppInternal>, _model: &Model<T>, update: impl FnOnce(&mut T))
    where
        T: 'static,
    {
        self.models.update_model::<T>(update);
    }
    
    pub fn to_host<C>(&self) -> Arc<C>
    where
        C: IToHost {
        self.to_hosts.get::<C>()
    }
}
