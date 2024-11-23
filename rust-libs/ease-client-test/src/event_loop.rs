use std::sync::{Arc, Mutex};

use ease_client::{Action, ViewAction};
use misty_vm::App;

#[derive(Clone)]
pub struct EventLoop {
    events: Arc<Mutex<Vec<ViewAction>>>,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            events: Default::default(),
        }
    }

    pub fn queue(&self, action: ViewAction) {
        let mut events = self.events.lock().unwrap();
        events.push(action);
    }

    pub fn flush(&self, app: &App) {
        loop {
            let mut events: Vec<ViewAction> = Default::default();

            {
                let mut w = self.events.lock().unwrap();
                std::mem::swap(&mut *w, &mut events);
            }

            if events.is_empty() {
                break;
            }

            for evt in events {
                app.emit(Action::View(evt));
            }
        }
    }
}
