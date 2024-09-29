use std::{
    any::Any,
    borrow::Borrow,
    cell::{Ref, RefCell},
    convert::Infallible,
    marker::PhantomData,
};

use misty_vm::{ViewModel, ViewModelContext};

use crate::{utils::{id_alloc::IdAlloc, lazy::Lazy}, AnyWidget, IntoAnyWidget, ObjectAction, WidgetContext, WidgetEvent};

use super::WidgetToHost;

pub struct WidgetViewModel<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    root: Lazy<AnyWidget>,
    atom_id_alloc: IdAlloc,
    ed_id_alloc: IdAlloc,
    event_mapper: fn(&Event) -> Option<&WidgetEvent>,
    _marker: PhantomData<(Event, E)>,
}

impl<Event, E> WidgetViewModel<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    pub fn new<R, W>(event_mapper: fn(&Event) -> Option<&WidgetEvent>, render_fn: R) -> Self
    where
        R: FnOnce() -> W + 'static,
        W: IntoAnyWidget,
    {
        Self {
            root: Lazy::new(move || render_fn().into_any()),
            atom_id_alloc: IdAlloc::new(),
            ed_id_alloc: IdAlloc::new(),
            event_mapper,
            _marker: Default::default(),
        }
    }

    fn init_root_impl(&self, cx: &mut WidgetContext, widget: &AnyWidget, parent_id: u64) {
        if widget.is_atom() {
            let (object, children) = widget.render_atom(cx);

            let id = cx.atom_id_alloc.allocate();
            cx.to_notify_objects.push(ObjectAction::Add { parent_id, id, data: object });

            for child in children.widgets().iter() {
                self.init_root_impl(cx, child, id);
            }
        } else {
            let widget = widget.render(cx);
            self.init_root_impl(cx, &widget, parent_id);
        }
    }

    fn init_root(&self, vm_cx: &ViewModelContext) {
        let r = self.root.get();
        let mut cx = WidgetContext {
            atom_id_alloc: self.atom_id_alloc.clone(),
            ed_id_alloc: self.ed_id_alloc.clone(),
            to_notify_objects: Default::default(),
        };
        self.init_root_impl(&mut cx, r.borrow(), 0);
        
        let mut to_notify_objects: Vec<ObjectAction> = Default::default();
        std::mem::swap(&mut cx.to_notify_objects, &mut to_notify_objects);
        vm_cx.to_host::<WidgetToHost>().notify_render_objects(to_notify_objects);
    }
}

impl<Event, E> ViewModel<Event, E> for WidgetViewModel<Event, E> {
    fn on_start(&self, cx: &ViewModelContext) -> Result<(), E> {
        self.init_root(cx);
        Ok(())
    }

    fn on_event(&self, cx: &ViewModelContext, event: &Event) -> Result<(), E> {
        let event = (self.event_mapper)(event);
        if let Some(event) = event {
            match event {
                WidgetEvent::Channel(id) => {
                    todo!()
                }
            }
        }
        Ok(())
    }
}
