use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    sync::Arc,
};

use crate::{async_task::MistyAsyncTaskContext, client::AsReadonlyMistyClientHandle};

pub trait MistyServiceTrait: Any + Send + Sync + Sized + 'static {
    fn of<'a>(cx: impl AsReadonlyMistyClientHandle<'a>) -> Arc<Self> {
        let services = &cx.readonly_handle().inner.service_manager;
        let service = services.services.get(&TypeId::of::<Self>());
        if service.is_none() {
            panic!(
                "service {} is not registered",
                std::any::type_name::<Self>()
            );
        }
        let service = service.unwrap().clone();
        let service = service.downcast::<Self>().unwrap();
        service
    }

    fn of_async<'a>(cx: &MistyAsyncTaskContext) -> Arc<Self> {
        let handle = cx.handle();
        let handle = handle.handle();
        Self::of(handle)
    }
}

pub struct MistyServiceManagerBuilder {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

#[derive(Debug)]
pub struct MistyServiceManager {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

pub enum ServiceImplPtr<T: ?Sized> {
    Boxed(Box<T>),
    Arc(Arc<T>),
}

impl MistyServiceManagerBuilder {
    pub fn new() -> Self {
        MistyServiceManagerBuilder {
            services: Default::default(),
        }
    }

    pub fn add<C>(mut self, service: C) -> Self
    where
        C: MistyServiceTrait,
    {
        let service = Arc::new(service);
        self.services.insert(TypeId::of::<C>(), service);
        self
    }

    pub fn build(self) -> MistyServiceManager {
        MistyServiceManager {
            services: self.services,
        }
    }
}

impl MistyServiceManager {
    pub fn builder() -> MistyServiceManagerBuilder {
        MistyServiceManagerBuilder::new()
    }
}
