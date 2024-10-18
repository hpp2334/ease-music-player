use misty_vm::{AppBuilderContext, ViewModel, ViewModelContext};

use crate::{actions::Action, error::EaseError};

pub struct PlaylistDetailVM {}

impl PlaylistDetailVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {}
    }
}

impl ViewModel<Action, EaseError> for PlaylistDetailVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        Ok(())
    }
}
