use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    rc::Rc,
};

use super::context::WidgetContext;

pub struct AnyWidget {}

pub trait IntoWidget {
    fn into_element(&self) -> AnyWidget {
        todo!()
    }
}

pub struct WidgetState<T> {
    state: Rc<RefCell<T>>,
}

impl<T> WidgetState<T> {
    pub fn get(&self, cx: &WidgetContext) -> Ref<'_, T> {
        todo!()
    }
    pub fn get_mut(&self, cx: &WidgetContext) -> RefMut<'_, T> {
        todo!()
    }
}

pub trait Widget {
    type State;
    type Props;

    fn state(&self) -> &WidgetState<Self::State> {
        todo!()
    }

    fn init_state() -> Self::State {
        todo!()
    }
}

pub trait WidgetAtom {
    fn update_render_object(&self, cx: &WidgetContext);
    fn children(&self) -> Vec<AnyWidget>;
}

pub trait WidgetRender {
    fn render(&self, cx: &WidgetContext) -> impl IntoWidget;
}

impl<T> IntoWidget for T where T: Widget {}
