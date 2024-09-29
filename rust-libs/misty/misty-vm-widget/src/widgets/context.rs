use crate::utils::id_alloc::IdAlloc;

use super::{
    AnyWidget, AnyWidgets, EventDispatcher, IntoAnyWidget, ObjectAction, RenderObject, WithRenderObject
};

pub struct WidgetContext {
    pub(crate) atom_id_alloc: IdAlloc,
    pub(crate) ed_id_alloc: IdAlloc,
    pub(crate) to_notify_objects: Vec<ObjectAction>
}

impl WidgetContext {
    pub fn event_dispatcher<Tp>(&self, handler: impl Fn(Tp) + 'static) -> EventDispatcher<Tp> {
        let id = self.ed_id_alloc.allocate();
        EventDispatcher::new(id, handler)
    }

    pub fn render_object(
        &self,
        o: impl RenderObject,
        children: AnyWidgets,
    ) -> WithRenderObject {
        WithRenderObject::new(o, children)
    }
}
