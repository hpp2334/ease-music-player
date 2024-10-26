use std::{any::Any, rc::Rc, sync::Arc};

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

pub(crate) trait BoxedViewModels {
    fn handle_event(&self, app: &Arc<AppInternal>, e: Box<dyn Any>);
}

pub(crate) struct ViewModels<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    vms: Vec<Box<dyn ViewModel<Event, E>>>,
}

impl<Event, E> ViewModels<Event, E> {
    pub fn new(vms: Vec<Box<dyn ViewModel<Event, E>>>) -> Self {
        Self { vms }
    }
}

impl<Event, E> BoxedViewModels for ViewModels<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    fn handle_event(&self, app: &Arc<AppInternal>, e: Box<dyn Any>) {
        let cx = ViewModelContext::new(app.clone());
        let evt = *e.downcast::<Event>().unwrap();

        for vm in self.vms.iter() {
            let res = vm.on_event(&cx, &evt);
            if let Err(_) = res {
                // TODO: error handler
                panic!("ViewModel on event error");
            }
        }
    }
}

pub(crate) struct DefaultBoxedViewModels;

impl BoxedViewModels for DefaultBoxedViewModels {
    fn handle_event(&self, _app: &Arc<AppInternal>, _e: Box<dyn Any>) {}
}
