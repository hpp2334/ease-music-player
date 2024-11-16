use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use ease_client_shared::backends::{music::MusicId, storage::StorageEntryLoc};

#[derive(Default, Clone)]
pub struct ConnectorState {
    pub port: u16,
    pub connector_handle: usize,
}

impl ConnectorState {
    pub fn serve_asset_url(&self, loc: StorageEntryLoc) -> String {
        let sp = URL_SAFE.encode(loc.path);
        let id: i64 = *loc.storage_id.as_ref();
        format!("http://127.0.0.1:{}/asset/{}?sp={}", self.port, id, sp)
    }

    pub fn serve_music_url(&self, id: MusicId) -> String {
        format!("http://127.0.0.1:{}/music/{}", self.port, id.as_ref())
    }

    pub fn serve_asset_url_opt(&self, loc: Option<StorageEntryLoc>) -> String {
        if let Some(loc) = loc {
            self.serve_asset_url(loc)
        } else {
            Default::default()
        }
    }
}
