use misty_vm::{App, AppBuilder};
use std::{any::Any, cell::RefCell, rc::Rc};

use crate::{utils::{opaque::Opaque, rc_owned::RcOwned}, EventDispatcherId, IntoAnyWidget, WidgetEvent, WidgetVMEvent};

use super::{WidgetToHost, WidgetViewModel};

pub trait WidgetAppBuilderExt<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    fn with_widget(
        self,
        vm: WidgetViewModel<Event, E>,
        to_host: WidgetToHost,
    ) -> AppBuilder<Event, E>;
}

pub trait WidgetAppExt {
    fn render<Event, F, R>(&self, event_mapper: fn(WidgetVMEvent) -> Event, widget: F)
    where
        Event: 'static,
        F: FnOnce() -> R + 'static,
        R: IntoAnyWidget;
    fn emit_widget_event<Event, Tp>(&self, event_mapper: fn(WidgetVMEvent) -> Event, ed_id: EventDispatcherId, payload: Tp)
    where
        Event: 'static,
        Tp: Any;
}

impl<Event, E> WidgetAppBuilderExt<Event, E> for AppBuilder<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    fn with_widget(
        self,
        vm: WidgetViewModel<Event, E>,
        to_host: WidgetToHost,
    ) -> AppBuilder<Event, E> {
        self.with_view_models(|ctx, builder| {
            builder.add(vm);
        })
        .with_to_hosts(|builder| {
            builder.add(to_host);
        })
    }
}

impl WidgetAppExt for App {
    fn render<Event, F, R>(&self, event_mapper: fn(WidgetVMEvent) -> Event, render_fn: F)
    where
        Event: 'static,
        F: FnOnce() -> R + 'static,
        R: IntoAnyWidget,
    {
        let event = WidgetVMEvent::InitRender(RcOwned::new(Box::new(|| render_fn().into_any())));
        let event = event_mapper(event);
        self.emit(event);
    }

    fn emit_widget_event<Event, Tp>(&self, event_mapper: fn(WidgetVMEvent) -> Event, ed_id: EventDispatcherId, payload: Tp)
    where
        Event: 'static,
        Tp: Any,
    {
        let event = WidgetEvent {
            ed_id,
            payload: Opaque::new(payload),
        };
        let event = WidgetVMEvent::Widget(RcOwned::new(event));
        let event = event_mapper(event);
        self.emit(event);
    }
}
