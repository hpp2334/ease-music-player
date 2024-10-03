use std::{
    any::Any,
    borrow::Borrow,
    cell::{Ref, RefCell},
    convert::Infallible,
    marker::PhantomData,
};

use misty_vm::{ViewModel, ViewModelContext};

use crate::{
    utils::{id_alloc::IdAlloc, lazy::Lazy},
    AnyWidget, IntoAnyWidget, ObjectAction, WidgetContext, WidgetEvent, WidgetVMEvent,
    WidgetVMStore,
};

use super::WidgetToHost;

pub struct WidgetViewModel<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    store: WidgetVMStore,
    event_mapper: fn(&Event) -> Option<&WidgetVMEvent>,
    _marker: PhantomData<(Event, E)>,
}

impl<Event, E> WidgetViewModel<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    pub fn new(event_mapper: fn(&Event) -> Option<&WidgetVMEvent>) -> Self {
        Self {
            store: WidgetVMStore::new(),
            event_mapper,
            _marker: Default::default(),
        }
    }

    fn build_widget_context(&self) -> WidgetContext {
        WidgetContext::new(self.store.clone())
    }
}

impl<Event, E> ViewModel<Event, E> for WidgetViewModel<Event, E> {
    fn on_event(&self, cx: &ViewModelContext, event: &Event) -> Result<(), E> {
        let event = (self.event_mapper)(event);
        let mut cx_widget = self.build_widget_context();
        let store = cx_widget.store.clone();
        if let Some(event) = event {
            match event {
                WidgetVMEvent::InitRender(f) => {
                    let f = f.take();
                    self.store.render_tree.init(&mut cx_widget, f);
                }
                WidgetVMEvent::Widget(evt) => {
                    let evt = evt.take();
                    let id = evt.ed_id;
                    let payload = evt.payload;
                    let event_dispatchers = RefCell::borrow(&self.store.event_dispatchers);
                    event_dispatchers.notify(&mut cx_widget, id, payload);
                }
            }
        }
        store.render_tree.rerender_if_dirty(&mut cx_widget);
        if !cx_widget.to_notify_objects.is_empty() {
            let mut objects: Vec<ObjectAction> = Default::default();
            std::mem::swap(&mut objects, &mut cx_widget.to_notify_objects);
            cx.to_host::<WidgetToHost>().notify_render_objects(objects);
        }
        Ok(())
    }
}
