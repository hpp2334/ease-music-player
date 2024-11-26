use std::{any::Any, sync::Arc};

use misty_async::AsyncRuntime;

use crate::{
    async_task::DefaultAsyncRuntimeAdapter, models::Models, to_host::ToHostsBuilder,
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
    cx: AppBuilderContext,
    view_models_builder: ViewModelsBuilder<Event>,
    to_hosts_builder: ToHostsBuilder,
    async_tasks: Arc<AsyncRuntime>,
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
        Self {
            cx: AppBuilderContext {
                models: Models::new(),
            },
            view_models_builder: ViewModelsBuilder::new(),
            to_hosts_builder: ToHostsBuilder::new(),
            async_tasks: AsyncRuntime::new(Arc::new(DefaultAsyncRuntimeAdapter)),
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

    pub fn with_async_runtime(mut self, rt: Arc<AsyncRuntime>) -> Self {
        self.async_tasks = rt;
        self
    }

    pub fn build(self) -> App {
        let pending_events = flume::unbounded::<Box<dyn Any>>();

        App {
            _app: Arc::new(AppInternal {
                thread_id: std::thread::current().id(),
                models: self.cx.models,
                view_models: self.view_models_builder.build(),
                to_hosts: self.to_hosts_builder.build(),
                async_executor: self.async_tasks,
                pending_events,
                during_flush: Default::default(),
            }),
        }
    }
}
