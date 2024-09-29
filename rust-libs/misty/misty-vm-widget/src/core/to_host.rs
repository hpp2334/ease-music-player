use misty_vm::misty_to_host;

use crate::ObjectAction;

pub trait IWidgetToHost: Send + Sync + 'static {
    fn notify_render_objects(&self, objects: Vec<ObjectAction>);
}
misty_to_host!(WidgetToHost, IWidgetToHost);