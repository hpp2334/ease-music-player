use std::{
    any::{Any, TypeId},
    error::Error,
    marker::PhantomData,
    rc::Rc,
};

use crate::utils::OMap;

use super::pod::{IAnyViewModel, ViewModel, ViewModels};

pub struct ViewModelsBuilder<Event>
where
    Event: Any + 'static,
{
    vms: OMap<TypeId, Rc<dyn IAnyViewModel>>,
    marker_event: PhantomData<Event>,
}

impl<Event> ViewModelsBuilder<Event>
where
    Event: Any + 'static,
{
    pub fn new() -> Self {
        Self {
            vms: Default::default(),
            marker_event: Default::default(),
        }
    }

    pub fn add<VM>(&mut self, vm: VM) -> &mut Self
    where
        VM: ViewModel,
    {
        let type_id = vm.type_id();
        if self.vms.contains_key(&type_id) {
            let name = std::any::type_name::<VM>();
            panic!("ViewModel {} already added", name);
        }
        self.vms.insert(std::any::TypeId::of::<VM>(), Rc::new(vm));
        self
    }

    pub(crate) fn build(self) -> ViewModels {
        let vms = ViewModels::new(self.vms);

        vms
    }
}
