use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
};
use ease_client_shared::backends::{
    connector::ConnectorAction,
    storage::{Storage, StorageId},
};
use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use super::state::AllStorageState;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum StorageCommonWidget {
    Create,
    Item { id: StorageId },
}

pub(crate) struct StorageCommonVM {
    pub store: Model<AllStorageState>,
}

impl StorageCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        StorageCommonVM { store: cx.model() }
    }

    fn sync_storages(&self, cx: &ViewModelContext, storages: Vec<Storage>) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.store);
        state.storage_ids = storages.iter().map(|v| v.id).collect();
        state.storages = storages.into_iter().map(|v| (v.id, v)).collect();
        Ok(())
    }
}

impl ViewModel for StorageCommonVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::Connector(action) => match action {
                ConnectorAction::Storages(storages) => {
                    self.sync_storages(cx, storages.clone())?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
