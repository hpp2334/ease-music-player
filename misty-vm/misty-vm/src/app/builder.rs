use std::{any::Any, sync::Arc};

use crate::{
    async_task::{AsyncTasks, DefaultAsyncRuntimeAdapter, IAsyncRuntimeAdapter},
    models::Models,
    to_host::{ToHosts, ToHostsBuilder},
    view_models::{BoxedViewModels, DefaultBoxedViewModels, ViewModelsBuilder},
    Model,
};

use super::{internal::AppInternal, pod::App};

pub struct AppBuilderContext {
    models: Models,
}

pub struct AppBuilder<Event, E>
where
    Event: 'static,
    E: 'static,
{
    cx: AppBuilderContext,
    view_models_builder: ViewModelsBuilder<Event, E>,
    to_hosts_builder: ToHostsBuilder,
    async_tasks: AsyncTasks,
}

impl AppBuilderContext {
    pub fn model<T>(&mut self) -> Model<T>
    where
        T: Default + 'static,
    {
        self.models.insert()
    }
}

impl<Event, E> AppBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            cx: AppBuilderContext {
                models: Models::new(),
            },
            view_models_builder: ViewModelsBuilder::new(),
            to_hosts_builder: ToHostsBuilder::new(),
            async_tasks: AsyncTasks::new(DefaultAsyncRuntimeAdapter),
        }
    }

    pub fn with_view_models(
        mut self,
        build: impl FnOnce(&mut AppBuilderContext, &mut ViewModelsBuilder<Event, E>),
    ) -> Self {
        build(&mut self.cx, &mut self.view_models_builder);
        self
    }

    pub fn with_to_hosts(mut self, build: impl FnOnce(&mut ToHostsBuilder)) -> Self {
        build(&mut self.to_hosts_builder);
        self
    }

    pub fn with_async_runtime_adapter(mut self, adapter: impl IAsyncRuntimeAdapter) -> Self {
        self.async_tasks = AsyncTasks::new(adapter);
        self
    }

    pub fn build(self) -> App {
        App {
            _app: Arc::new(AppInternal {
                models: self.cx.models,
                view_models: self.view_models_builder.build(),
                to_hosts: self.to_hosts_builder.build(),
                async_tasks: self.async_tasks,
            }),
        }
    }
}
