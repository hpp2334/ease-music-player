use super::{EventDispatcher, IntoWidget};

pub struct WidgetContext {}

impl WidgetContext {
    pub fn event_dispatcher<Tp>(&self, handler: impl Fn(Tp)) -> EventDispatcher<Tp> {
        todo!()
    }
}
