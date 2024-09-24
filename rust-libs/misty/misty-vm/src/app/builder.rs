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

pub struct AppBuilder {
    cx: AppBuilderContext,
    view_models: Box<dyn BoxedViewModels>,
    to_hosts: ToHosts,
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

impl AppBuilder {
    pub(crate) fn new() -> Self {
        Self {
            cx: AppBuilderContext {
                models: Models::new(),
            },
            view_models: Box::new(DefaultBoxedViewModels),
            to_hosts: ToHostsBuilder::new().build(),
            async_tasks: AsyncTasks::new(DefaultAsyncRuntimeAdapter),
        }
    }

    pub fn with_view_models<Event, E>(
        mut self,
        build: impl FnOnce(
            &mut AppBuilderContext,
            ViewModelsBuilder<Event, E>,
        ) -> ViewModelsBuilder<Event, E>,
    ) -> Self
    where
        Event: Any + 'static,
        E: Any + 'static,
    {
        let builder = ViewModelsBuilder::new();
        self.view_models = build(&mut self.cx, builder).build();
        self
    }

    pub fn with_to_hosts(mut self, build: impl FnOnce(ToHostsBuilder) -> ToHostsBuilder) -> Self {
        let builder = ToHostsBuilder::new();
        self.to_hosts = build(builder).build();
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
                view_models: self.view_models,
                to_hosts: self.to_hosts,
                async_tasks: self.async_tasks,
            }),
        }
    }
}
