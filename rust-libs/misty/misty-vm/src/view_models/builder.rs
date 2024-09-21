use std::any::Any;

use super::pod::{IViewModel, IViewModels, ViewModels};

pub struct ViewModelsBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    vms: Vec<Box<dyn IViewModel<Event, E>>>,
}

impl<Event, E> ViewModelsBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    pub fn new() -> Self {
        Self {
            vms: Default::default(),
        }
    }

    pub fn add(mut self, vm: impl IViewModel<Event, E>) -> Self {
        self.vms.push(Box::new(vm));
        self
    }

    pub(crate) fn build(self) -> Box<dyn IViewModels> {
        let vms = ViewModels::new(self.vms);

        Box::new(vms)
    }
}
