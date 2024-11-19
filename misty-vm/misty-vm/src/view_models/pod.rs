use std::{
    any::{Any, TypeId},
    fmt::Debug,
    rc::Rc,
    sync::Arc,
};

use tracing::instrument;

use crate::{internal::AppInternal, utils::OMap, IntoVMError};

use super::context::ViewModelContext;

pub trait ViewModel
where
    Self: Any + 'static,
{
    type Event: Any + 'static;
    type Error: IntoVMError + 'static;

    fn of(cx: &ViewModelContext) -> Rc<Self>
    where
        Self: Sized,
    {
        cx.app().view_models.vm::<Self>()
    }

    fn on_flush(&self, cx: &ViewModelContext) -> Result<(), Self::Error> {
        let _ = cx;
        Ok(())
    }

    fn on_event(&self, cx: &ViewModelContext, event: &Self::Event) -> Result<(), Self::Error>;
}

pub(crate) trait IAnyViewModel
where
    Self: 'static,
{
    fn as_any(self: Rc<Self>) -> Rc<dyn Any>;
    fn handle_flush(&self, cx: &ViewModelContext) -> Result<(), Box<dyn std::error::Error>>;
    fn handle_event(
        &self,
        cx: &ViewModelContext,
        event: &dyn Any,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl<VM> IAnyViewModel for VM
where
    VM: ViewModel,
{
    fn as_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn handle_flush(&self, cx: &ViewModelContext) -> Result<(), Box<dyn std::error::Error>> {
        self.on_flush(cx).map_err(|e| e.cast())
    }

    fn handle_event(
        &self,
        cx: &ViewModelContext,
        event: &dyn Any,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let vm_name = std::any::type_name::<VM>();
        let event_name = std::any::type_name::<VM::Event>();
        let event = event
            .downcast_ref::<VM::Event>()
            .expect(format!("failed to downcast event {} for VM {}", event_name, vm_name).as_str());
        self.on_event(cx, event).map_err(|e| e.cast())
    }
}

pub(crate) struct ViewModels {
    vms: OMap<TypeId, Rc<dyn IAnyViewModel>>,
}
impl std::fmt::Debug for ViewModels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewModels").finish()
    }
}

impl ViewModels {
    pub fn new(vms: OMap<TypeId, Rc<dyn IAnyViewModel>>) -> Self {
        Self { vms }
    }

    #[instrument]
    pub fn handle_event(&self, app: &Arc<AppInternal>, evt: &dyn Any) {
        let cx = ViewModelContext::new(app.clone());

        for (_, vm) in self.vms.iter() {
            let res = vm.handle_event(&cx, evt);
            if let Err(e) = res {
                panic!("ViewModel on event error: {}", e);
            }
        }

        tracing::trace!("end");
    }

    #[instrument]
    pub fn handle_flush(&self, app: &Arc<AppInternal>) {
        tracing::trace!("start");

        let cx = ViewModelContext::new(app.clone());
        for (_, vm) in self.vms.iter() {
            let res = vm.handle_flush(&cx);
            if let Err(e) = res {
                panic!("ViewModel on flush error: {}", e);
            }
        }

        tracing::trace!("end");
    }

    pub fn vm<VM>(&self) -> Rc<VM>
    where
        VM: ViewModel,
    {
        let vm = self.vms.get(&TypeId::of::<VM>());

        if let Some(vm) = vm {
            let name = std::any::type_name::<VM>();
            vm.clone()
                .as_any()
                .downcast::<VM>()
                .expect(format!("failed to downcast {}", name).as_str())
        } else {
            let name = std::any::type_name::<VM>();
            panic!("ViewModel {} not found", name)
        }
    }
}
