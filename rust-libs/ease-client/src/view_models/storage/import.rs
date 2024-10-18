use ease_client_shared::{
    backends::{music::MusicId, playlist::PlaylistId},
    uis::{music::ArgSeekMusic, storage::CurrentStorageImportType},
};
use misty_vm::{AppBuilderContext, IToHost, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    to_host::router::{RouteKey, RouterService},
};

use super::state::CurrentStorageState;

pub struct StorageImportVM {
    state: Model<CurrentStorageState>,
}

impl StorageImportVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self { state: cx.model() }
    }

    pub(crate) fn route(
        &self,
        cx: &ViewModelContext,
        typ: CurrentStorageImportType,
    ) -> EaseResult<()> {
        RouterService::of(cx).route_storage();

        Ok(())
    }
}

impl ViewModel<Action, EaseError> for StorageImportVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        Ok(())
    }
}
