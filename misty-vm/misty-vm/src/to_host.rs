use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    sync::Arc,
};

pub trait IToHost: Any + Send + Sync + Sized + 'static {}

pub struct ToHostsBuilder {
    to_hosts: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

#[derive(Debug)]
pub struct ToHosts {
    to_hosts: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

pub enum ToHostImplPtr<T: ?Sized> {
    Boxed(Box<T>),
    Arc(Arc<T>),
}

impl ToHostsBuilder {
    pub(crate) fn new() -> Self {
        ToHostsBuilder {
            to_hosts: Default::default(),
        }
    }

    pub fn add<C>(&mut self, to_host: C) -> &mut Self
    where
        C: IToHost,
    {
        let to_host = Arc::new(to_host);
        self.to_hosts.insert(TypeId::of::<C>(), to_host);
        self
    }

    pub(crate) fn build(self) -> ToHosts {
        ToHosts {
            to_hosts: self.to_hosts,
        }
    }
}

impl ToHosts {
    pub fn get<C>(&self) -> Arc<C>
    where
        C: IToHost,
    {
        let s = self
            .to_hosts
            .get(&TypeId::of::<C>())
            .expect(format!("ToHost {} not registered", std::any::type_name::<C>()).as_str());
        let s = s.clone().downcast::<C>().unwrap();
        s
    }
}
