use std::{any::{Any, TypeId}, collections::{HashMap, HashSet}, marker::PhantomData};

use super::{pod::{BoxedViewModel, BoxedViewModels}, ViewModel};


pub struct ViewModelsBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    vms: HashMap<TypeId, BoxedViewModel>,
    vm_ids: HashSet<TypeId>,
    marker_event: PhantomData<Event>,
    marker_error: PhantomData<E>,
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
            marker_event: Default::default(),
            marker_error: Default::default()
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
        self.vms.insert(std::any::TypeId::of::<VM>(), BoxedViewModel::new(vm));
        self
    }

    pub(crate) fn build(self) -> BoxedViewModels {
        let vms = BoxedViewModels::new(self.vms);

        vms
    }
}
