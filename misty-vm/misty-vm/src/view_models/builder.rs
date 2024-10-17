use std::any::Any;

use super::pod::{BoxedViewModels, ViewModel, ViewModels};

pub struct ViewModelsBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    vms: Vec<Box<dyn ViewModel<Event, E>>>,
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

    pub fn add(&mut self, vm: impl ViewModel<Event, E>) -> &mut Self {
        self.vms.push(Box::new(vm));
        self
    }

    pub(crate) fn build(self) -> Box<dyn BoxedViewModels> {
        let vms = ViewModels::new(self.vms);

        Box::new(vms)
    }
}
