use ease_client_shared::{
    backends::{music::MusicId, playlist::PlaylistId},
    uis::music::ArgSeekMusic,
};
use misty_vm::{AppBuilderContext, ViewModel, ViewModelContext};

use crate::{actions::Action, error::EaseError};

pub enum PlaylistPageAction {}

pub struct PlaylistDetailVM {}

impl PlaylistDetailVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {}
    }

    pub fn id(&self) -> Option<PlaylistId> {
        todo!()
    }
}

impl ViewModel<Action, EaseError> for PlaylistDetailVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {}
}
