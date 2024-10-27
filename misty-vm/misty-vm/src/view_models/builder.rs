use std::{any::{Any, TypeId}, collections::HashSet};

use super::pod::{BoxedViewModels, ViewModel, ViewModels};

pub struct ViewModelsBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    vms: Vec<Box<dyn ViewModel<Event, E>>>,
    vm_ids: HashSet<TypeId>
}

impl<Event, E> ViewModelsBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    pub fn new() -> Self {
        Self {
            vms: Default::default(),
            vm_ids: Default::default(),
        }
    }

    pub fn add<VM>(&mut self, vm: VM) -> &mut Self
    where VM: ViewModel<Event, E> {
        let type_id = vm.type_id();
        if self.vm_ids.contains(&type_id) {
            let name = std::any::type_name::<VM>();
            panic!("ViewModel {} already added", name);
        }
        self.vm_ids.insert(type_id);
        self.vms.push(Box::new(vm));
        self
    }

    pub(crate) fn build(self) -> Box<dyn BoxedViewModels> {
        let vms = ViewModels::new(self.vms);

        Box::new(vms)
    }
}
