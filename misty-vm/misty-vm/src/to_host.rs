use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    rc::Rc,
    sync::Arc,
};

use misty_lifecycle::{ArcLocal, ArcLocalAny, ArcLocalCore};

use crate::view_models::context::ViewModelContext;

pub trait IToHost: Any + Sized + 'static {
    fn of(cx: &ViewModelContext) -> Rc<Self> {
        cx.app().to_hosts.get()
    }
}

pub struct ToHostsBuilder {
    core: ArcLocalCore,
    to_hosts: HashMap<TypeId, ArcLocalAny>,
}

#[derive(Debug)]
pub struct ToHosts {
    to_hosts: HashMap<TypeId, ArcLocalAny>,
}

pub enum ToHostImplPtr<T: ?Sized> {
    Boxed(Box<T>),
    Arc(Arc<T>),
}

impl ToHostsBuilder {
    pub(crate) fn new(core: ArcLocalCore) -> Self {
        ToHostsBuilder {
            core,
            to_hosts: Default::default(),
        }
    }

    pub fn add<C>(&mut self, to_host: C) -> &mut Self
    where
        C: IToHost,
    {
        let to_host = ArcLocal::new(self.core, to_host).as_any();
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
    pub fn get<C>(&self) -> Rc<C>
    where
        C: IToHost,
    {
        let s = self
            .to_hosts
            .get(&TypeId::of::<C>())
            .expect(format!("ToHost {} not registered", std::any::type_name::<C>()).as_str());
        let s = s.clone().try_downcast::<C>().unwrap();
        s.get().clone()
    }
}
