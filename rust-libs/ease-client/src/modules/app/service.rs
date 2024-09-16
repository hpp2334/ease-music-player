use std::sync::Arc;

use ease_client_backend::Backend;
use ease_client_shared::backends::app::ArgInitializeApp;
use misty_vm::{client::MistyClientHandle, states::MistyStateTrait, MistyState};

use crate::modules::error::{EaseError, EaseResult};

#[derive(Default, Clone, MistyState)]
pub struct BackendState {
    pub backend: Option<Arc<Backend>>,
}

pub fn init_backend(cx: MistyClientHandle, arg: ArgInitializeApp) -> EaseResult<()> {
    let backend = Backend::new(arg).map_err(|e| EaseError::BackendInitFail(e))?;
    let backend = Arc::new(backend);

    BackendState::update(cx, move |state| {
        state.backend = Some(backend);
    });
    Ok(())
}

pub fn get_backend(cx: MistyClientHandle) -> Arc<Backend> {
    let backend = BackendState::map(cx, |state| state.backend.clone());
    if backend.is_none() {
        panic!("backend is none");
    }
    return backend.unwrap();
}
