use std::{any::Any, sync::Arc};

use misty_lifecycle::{ArcLocalCore, ILifecycleExternal, Lifecycle};

use crate::{
    async_task::DefaultLifecycleExternal, models::Models, to_host::ToHostsBuilder,
    view_models::builder::ViewModelsBuilder, Model,
};

use super::{internal::AppInternal, pod::App};

pub struct AppBuilderContext {
    models: Models,
}

pub struct AppBuilder<Event>
where
    Event: 'static,
{
    local: ArcLocalCore,
    cx: AppBuilderContext,
    view_models_builder: ViewModelsBuilder<Event>,
    to_hosts_builder: ToHostsBuilder,
    lifecycle: Arc<Lifecycle>,
}

impl AppBuilderContext {
    pub fn model<T>(&mut self) -> Model<T>
    where
        T: Default + 'static,
    {
        self.models.insert()
    }
}

impl<Event> AppBuilder<Event>
where
    Event: Any + 'static,
{
    pub(crate) fn new() -> Self {
        let local = ArcLocalCore::new();

        Self {
            local,
            cx: AppBuilderContext {
                models: Models::new(),
            },
            view_models_builder: ViewModelsBuilder::new(),
            to_hosts_builder: ToHostsBuilder::new(local),
            lifecycle: Lifecycle::new(Arc::new(DefaultLifecycleExternal)),
        }
    }

    pub fn with_view_models(
        mut self,
        build: impl FnOnce(&mut AppBuilderContext, &mut ViewModelsBuilder<Event>),
    ) -> Self {
        build(&mut self.cx, &mut self.view_models_builder);
        self
    }

    pub fn with_to_hosts(mut self, build: impl FnOnce(&mut ToHostsBuilder)) -> Self {
        build(&mut self.to_hosts_builder);
        self
    }

    pub fn with_async_dispatcher(mut self, dispatcher: Arc<dyn ILifecycleExternal>) -> Self {
        self.lifecycle = Lifecycle::new(dispatcher);
        self
    }

    pub fn build(self) -> App {
        let pending_events = flume::unbounded::<Box<dyn Any>>();

        App {
            _app: Arc::new(AppInternal {
                local: self.local,
                models: self.cx.models,
                view_models: self.view_models_builder.build(),
                to_hosts: self.to_hosts_builder.build(),
                async_executor: self.lifecycle,
                pending_events,
                during_flush: Default::default(),
            }),
        }
    }
}
