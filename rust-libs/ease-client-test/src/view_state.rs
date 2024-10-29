use std::sync::{Arc, Mutex};

use ease_client::{IViewStateService, RootViewModelState};

#[derive(Clone)]
pub struct ViewStateServiceRef {
    state: Arc<Mutex<RootViewModelState>>,
}

impl ViewStateServiceRef {
    pub fn new() -> Self {
        Self {
            state: Default::default(),
        }
    }

    pub fn state(&self) -> RootViewModelState {
        let state = self.state.lock().unwrap();
        state.clone()
    }
}

impl IViewStateService for ViewStateServiceRef {
    fn notify(&self, v: RootViewModelState) {
        let mut state = self.state.lock().unwrap();
        state.merge_from(v);
    }
}
