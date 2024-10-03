use crate::utils::clonable_opaque::ClonableOpaque;
use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    rc::Rc,
};

use super::{
    context::WidgetContext, props::AnyProps, AnyRenderObject, AnyWidgetState, RenderObject,
    WidgetState,
};

#[derive(Clone)]
pub struct AnyWidgetData {
    type_id: TypeId,
    props: AnyProps,
    state: AnyWidgetState,
}


impl AnyWidgetData {
    pub fn new<T, P, S>(props: P, state: S) -> Self
    where
        T: 'static,
        P: Clone + Any + 'static,
        S: Clone + 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        AnyWidgetData {
            type_id,
            props: AnyProps::new(props),
            state: AnyWidgetState::new(state),
        }
    }
}

pub struct AnyWidget {
    widget: Rc<dyn WidgetInternal>,
}

#[derive(Clone, Default)]
pub struct AnyWidgets {
    widgets: Rc<RefCell<Vec<AnyWidget>>>,
}

impl AnyWidgets {
    pub fn new() -> Self {
        Self {
            widgets: Default::default(),
        }
    }

    pub fn push(&self, widget: impl IntoAnyWidget) {
        self.widgets.borrow_mut().push(widget.into_any());
    }

    pub fn widgets(&self) -> Ref<'_, [AnyWidget]> {
        Ref::map(self.widgets.borrow(), |widgets| widgets.as_slice())
    }
}

pub trait WidgetMeta: 'static {
    type State;
    type Props;

    fn data(&self) -> &AnyWidgetData;
    fn data_mut(&mut self) -> &mut AnyWidgetData;
    fn init_state() -> Self::State;
}

pub trait WidgetPropsState {
    type State;
    type Props;

    fn props(&self) -> &Self::Props;
    fn state(&self) -> WidgetState<Self::State>;
}

impl<T, P, S> WidgetPropsState for T
where
    T: WidgetMeta<State = S, Props = P>,
    P: Clone + 'static,
    S: 'static,
{
    type Props = T::Props;
    type State = T::State;

    fn props(&self) -> &Self::Props {
        self.data().props.downcast_ref::<Self::Props>()
    }
    fn state(&self) -> WidgetState<Self::State> {
        WidgetState::wrap(self.data().state.clone())
    }
}

#[macro_export]
macro_rules! define_widget {
    ($ty:ident,$props:ty,$state:ty,$init_state:expr) => {
        struct $ty {
            data: misty_vm_widget::AnyWidgetData,
        }

        impl $ty {
            pub fn build(props: $props) -> Self {
                let state = Self::init_state();
                let data = misty_vm_widget::AnyWidgetData::new::<Self, _, _>(props, state);

                Self { data }
            }
        }

        impl WidgetMeta for $ty {
            type Props = $props;
            type State = $state;

            fn data(&self) -> &misty_vm_widget::AnyWidgetData {
                &self.data
            }
            fn data_mut(&mut self) -> &mut misty_vm_widget::AnyWidgetData {
                &mut self.data
            }

            fn init_state() -> Self::State {
                $init_state()
            }
        }
    };
    ($ty:ident,$props:ty) => {
        define_widget!($ty, $props, (), misty_vm_widget::init_state_unit);
    };
}

impl AnyWidget {
    fn new(widget: impl WidgetInternal + 'static) -> Self {
        Self {
            widget: Rc::new(widget),
        }
    }

    pub(crate) fn clone_widget(&self) -> Self {
        Self {
            widget: self.widget.clone(),
        }
    }

    pub fn is_atom(&self) -> bool {
        self.widget.widget_type_id() == std::any::TypeId::of::<WithRenderObject>()
    }

    pub fn render(&self, cx: &WidgetContext) -> AnyWidget {
        debug_assert!(!self.is_atom());
        self.widget.render(cx)
    }

    pub fn render_atom(&self, cx: &WidgetContext) -> (AnyRenderObject, AnyWidgets) {
        debug_assert!(self.is_atom());

        let data = self
            .widget
            .data()
            .props
            .downcast_ref::<WithRenderObjectData>();
        let object = data.object.clone();
        let children = data.children.clone();

        (object, children)
    }
}

pub trait IntoAnyWidget {
    fn into_any(self) -> AnyWidget;
}

pub fn init_state_unit() -> () {
    ()
}

pub trait Widget: WidgetMeta {
    fn render(&self, cx: &WidgetContext) -> impl IntoAnyWidget;
}

trait WidgetInternal {
    fn widget_type_id(&self) -> TypeId;
    fn data(&self) -> &AnyWidgetData;
    fn render(&self, cx: &WidgetContext) -> AnyWidget;
}

impl<T> WidgetInternal for T
where
    T: Widget,
{
    fn widget_type_id(&self) -> TypeId {
        std::any::TypeId::of::<T>()
    }

    fn data(&self) -> &AnyWidgetData {
        WidgetMeta::data(self)
    }

    fn render(&self, cx: &WidgetContext) -> AnyWidget {
        Widget::render(self, cx).into_any()
    }
}

impl<T> IntoAnyWidget for T
where
    T: Widget + 'static,
{
    fn into_any(self) -> AnyWidget {
        AnyWidget::new(self)
    }
}

#[derive(Clone)]
struct WithRenderObjectData {
    object: AnyRenderObject,
    children: AnyWidgets,
}

pub struct WithRenderObject {
    data: AnyWidgetData,
}

impl WithRenderObject {
    pub(crate) fn new(o: impl RenderObject, children: AnyWidgets) -> Self {
        let object = o.into_any();

        Self {
            data: AnyWidgetData {
                type_id: std::any::TypeId::of::<WithRenderObject>(),
                props: AnyProps::new(WithRenderObjectData { object, children }),
                state: AnyWidgetState::new(()),
            },
        }
    }
}
impl WidgetInternal for WithRenderObject {
    fn widget_type_id(&self) -> TypeId {
        std::any::TypeId::of::<WithRenderObject>()
    }

    fn data(&self) -> &AnyWidgetData {
        &self.data
    }

    fn render(&self, cx: &WidgetContext) -> AnyWidget {
        panic!("WithRenderObject cannot be rendered");
    }
}

impl IntoAnyWidget for WithRenderObject {
    fn into_any(self) -> AnyWidget {
        AnyWidget::new(self)
    }
}

pub(crate) struct WidgetRootPlaceholder;
impl WidgetInternal for WidgetRootPlaceholder {
    fn widget_type_id(&self) -> TypeId {
        panic!("cannot get WidgetRoot type_id")
    }

    fn data(&self) -> &AnyWidgetData {
        panic!("cannot get WidgetRoot data")
    }

    fn render(&self, cx: &WidgetContext) -> AnyWidget {
        panic!("cannot render WidgetRoot")
    }
}
impl WidgetRootPlaceholder {
    pub fn new_any() -> AnyWidget {
        AnyWidget::new(WidgetRootPlaceholder)
    }
}

pub(crate) struct RenderObjectPlaceholder;
impl WidgetInternal for RenderObjectPlaceholder {
    fn widget_type_id(&self) -> TypeId {
        panic!("cannot get RenderObjectPlaceholder type_id")
    }

    fn data(&self) -> &AnyWidgetData {
        panic!("cannot get RenderObjectPlaceholder data")
    }

    fn render(&self, cx: &WidgetContext) -> AnyWidget {
        panic!("cannot render RenderObjectPlaceholder")
    }
}
impl RenderObjectPlaceholder {
    pub fn new_any() -> AnyWidget {
        AnyWidget::new(RenderObjectPlaceholder)
    }
}
