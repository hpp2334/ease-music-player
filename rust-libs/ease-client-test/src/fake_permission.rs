use std::sync::{atomic::AtomicBool, Arc};

use ease_client::{IPermissionService, MainAction, ViewAction};

use crate::event_loop::EventLoop;

#[derive(Clone)]
pub struct FakePermissionService {
    _value: Arc<AtomicBool>,
    _have_requested: Arc<AtomicBool>,
    event_loop: EventLoop,
}
impl FakePermissionService {
    pub fn new(event_loop: EventLoop) -> Self {
        Self {
            _value: Default::default(),
            _have_requested: Default::default(),
            event_loop,
        }
    }

    pub fn update_permission(&self, value: bool) {
        self._value
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn have_requested(&self) -> bool {
        self._have_requested
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
impl IPermissionService for FakePermissionService {
    fn have_storage_permission(&self) -> bool {
        self._value.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn request_storage_permission(&self) {
        self._have_requested
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.event_loop
            .queue(ViewAction::Main(MainAction::PermissionChanged));
    }
}
