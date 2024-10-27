use ease_client_shared::backends::storage::StorageEntryLoc;
use base64::{engine::general_purpose::URL_SAFE, Engine as _};


#[derive(Default, Clone)]
pub struct ConnectorState {
    pub port: u16,
}

impl ConnectorState {
    pub fn serve_url(&self, loc: StorageEntryLoc) -> String {
        let sp = URL_SAFE.encode(loc.path);
        let id: i64 = *loc.storage_id.as_ref();
        format!("http://127.0.0.1:{}/asset/{}?sp={}", self.port, id, sp)
    }

    pub fn serve_url_opt(&self, loc: Option<StorageEntryLoc>) -> String {
        if let Some(loc) = loc {
            self.serve_url(loc)
        } else {
            Default::default()
        }
    }
}