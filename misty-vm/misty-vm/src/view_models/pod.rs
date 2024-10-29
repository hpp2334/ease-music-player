use std::{any::{Any, TypeId}, collections::HashMap, rc::Rc, sync::Arc};

use crate::internal::AppInternal;

use super::ViewModelContext;

pub trait ViewModel<Event, E>: 'static
where
    E: Any + 'static,
{
    fn of(cx: &ViewModelContext) -> Rc<Self>
    where
        Self: Sized,
    {
        cx.vm::<Self, _, _>()
    }

    fn on_event(&self, cx: &ViewModelContext, event: &Event) -> Result<(), E>;
}

#[derive(Clone)]
pub(crate) struct BoxedViewModel {
    internal: Rc<dyn Any>
}


pub(crate) struct BoxedViewModels {
    vms: HashMap<TypeId, BoxedViewModel>
}

impl BoxedViewModel {
    pub fn new<VM, Event, E>(vm: VM) -> Self
    where VM: ViewModel<Event, E>,
    Event: Any + 'static,
    E: Any + 'static, {
        Self {
            internal: Rc::new(vm)
        }
    }

    pub fn on_event<Event, E>(&self, cx: &ViewModelContext, event: &Event) -> Result<(), E>
    where Event: Any + 'static,
    E: Any + 'static, {
        todo!()
    }

    pub fn to<VM, Event, E>(&self) -> Rc<VM>
    where VM: ViewModel<Event, E>,
    Event: Any + 'static,
    E: Any + 'static, {
        let internal = self.internal.clone().downcast::<VM>().unwrap();
        internal
    }
}


impl BoxedViewModels {
    pub fn new(vms: HashMap<TypeId, BoxedViewModel>) -> Self {
        Self {
            vms
        }
    }

    pub fn handle_event<Event, E>(&self, app: &Arc<AppInternal>, evt: Event)
    where
        Event: Any + 'static,
        E: Any + 'static, {
        let cx = ViewModelContext::new(app.clone());

        for (_, vm) in self.vms.iter() {
            let res = vm.on_event::<Event, E>(&cx, &evt);
            if let Err(_) = res {
                // TODO: error handler
                panic!("ViewModel on event error");
            }
        }
    }
}
