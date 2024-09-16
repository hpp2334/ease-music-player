use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use ease_client_shared::backends::{music::MusicId, storage::StorageEntryLoc};

use crate::ctx::Context;

fn base_url(cx: &Context) -> String {
    let port = cx.server_port.load(std::sync::atomic::Ordering::Relaxed);
    format!("http://127.0.0.1:{}", port)
}

pub fn get_serve_url_from_loc(cx: &Context, loc: StorageEntryLoc) -> String {
    let sp = URL_SAFE.encode(loc.path);
    let id: i64 = *loc.storage_id.as_ref();
    format!("{}/asset/{}?sp={}", base_url(cx), id, sp)
}

pub fn get_serve_url_from_opt_loc(cx: &Context, loc: Option<StorageEntryLoc>) -> String {
    if let Some(loc) = loc {
        get_serve_url_from_loc(cx, loc)
    } else {
        Default::default()
    }
}

pub fn get_serve_url_from_music_id(cx: &Context, id: MusicId) -> String {
    let id: i64 = *id.as_ref();
    format!("{}/music/{}", base_url(cx), id)
}
