use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use ease_client_shared::backends::{
    music::MusicId,
    storage::{DataSourceKey, StorageEntryLoc},
};

#[derive(Default, Clone)]
pub struct ConnectorState {
    pub connector_handle: usize,
}

impl ConnectorState {}
