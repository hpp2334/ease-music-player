use std::{
    any::Any,
    borrow::Borrow,
    cell::{Ref, RefCell},
    convert::Infallible,
    marker::PhantomData,
};

use misty_vm::{ViewModel, ViewModelContext};

use crate::{AnyWidget, IntoWidget, WidgetEvent};

struct Lazy<T> {
    f: RefCell<Option<Box<dyn FnOnce() -> T>>>,
    value: RefCell<Option<T>>,
}

impl<T> Lazy<T> {
    pub fn new(f: impl FnOnce() -> T + 'static) -> Self {
        Self {
            f: RefCell::new(Some(Box::new(f))),
            value: RefCell::new(None),
        }
    }

    pub fn get(&self) -> Ref<'_, T> {
        let uninit = self.value.borrow().is_none();
        if uninit {
            let f = self.f.borrow_mut().take().expect("init function is None");
            let value = f();
            *self.value.borrow_mut() = Some(value);
        }
        let v = Ref::map(self.value.borrow(), |v| v.as_ref().expect("value is None"));
        v
    }
}

pub struct WidgetViewModel<Event, E>
where
    Event: Any + 'static,
    E: Any + 'static,
{
    root: Lazy<AnyWidget>,
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
        W: IntoWidget,
    {
        Self {
            root: Lazy::new(move || render_fn().into_any()),
            event_mapper,
            _marker: Default::default(),
        }
    }

    fn render_root(&self) {
        self.root.get();
    }
}

impl<Event, E> ViewModel<Event, E> for WidgetViewModel<Event, E> {
    fn on_start(&self) -> Result<(), E> {
        self.render_root();
        Ok(())
    }

    fn on_event(&self, cx: &ViewModelContext, event: &Event) -> Result<(), E> {
        let event = (self.event_mapper)(event);
        if let Some(event) = event {
            match event {
                WidgetEvent::RenderRoot => {
                    self.render_root();
                }
                WidgetEvent::Channel(id) => {
                    todo!()
                }
            }
        }
        Ok(())
    }
}
