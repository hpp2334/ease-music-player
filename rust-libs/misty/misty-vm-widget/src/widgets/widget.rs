use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    rc::Rc,
};

use super::context::WidgetContext;

pub struct AnyWidget {}

pub struct EmptyWidget {}


pub trait IntoWidget {
    fn into_any(&self) -> AnyWidget {
        todo!()
    }
}
 
pub trait AsWidget {
    fn as_any(&self) -> &AnyWidget {
        todo!()
    }
}
impl AsWidget for &AnyWidget {
    
}

impl Widget for EmptyWidget {
    type Props = ();
    type State = ();

    fn state(&self) -> &WidgetState<Self::State> {
        std::todo!()
    }

    fn init_state() -> Self::State {
        std::todo!()
    }
}

pub type EmptyWidgets = Vec<EmptyWidget>;

pub fn empty_widgets() -> Vec<EmptyWidget> {
    vec![]
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

pub trait RenderObject {}

pub trait WidgetAtom {
    fn render_object(&self, cx: &WidgetContext) -> impl RenderObject;
    fn children(&self) -> impl Iterator<Item = impl AsWidget>;
}

pub trait WidgetRender {
    fn render(&self, cx: &WidgetContext) -> impl IntoWidget;
}


impl<T> IntoWidget for T where T: Widget {}
impl<T> AsWidget for &T where T: Widget {
    fn as_any(&self) -> &AnyWidget {
        std::todo!()
    }
}