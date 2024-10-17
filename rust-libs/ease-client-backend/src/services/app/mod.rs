use std::sync::Arc;

use ease_client_shared::backends::{app::ArgInitializeApp, preference::PreferenceData};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::Level;

use crate::{ctx::BackendContext, error::BResult, repositories::core::get_conn};

use super::server::start_server;

#[derive(Debug, Serialize, Deserialize)]
struct AppMeta {
    pub schema_version: u32,
    pub upgrading: bool,
}

fn load_persistent_data<T: Serialize + DeserializeOwned>(
    app_document_dir: &str,
    name: &str,
) -> Option<T> {
    let path = app_document_dir.to_string() + name;

    if std::fs::metadata(&path).is_err() {
        return None;
    }
    let data = std::fs::read(path).unwrap();
    let data = serde_json::from_slice::<T>(&data);
    match data {
        Ok(data) => Some(data),
        Err(e) => {
            tracing::error!(
                "deserialize {} to json error: {}",
                std::any::type_name::<T>(),
                e
            );
            None
        }
    }
}

fn save_persistent_data<'a, T: Serialize + DeserializeOwned>(
    app_document_dir: &str,
    name: &str,
    data: T,
) {
    let path = app_document_dir.to_string() + name;

    let data = serde_json::to_string(&data).unwrap();
    std::fs::write(path, data).unwrap();
}

fn load_app_meta(app_document_dir: &str) -> AppMeta {
    let meta = load_persistent_data::<AppMeta>(app_document_dir, "meta.json");
    match meta {
        Some(meta) => meta,
        None => AppMeta {
            schema_version: 0,
            upgrading: false,
        },
    }
}

fn save_current_app_meta(cx: &BackendContext) {
    save_persistent_data(
        &cx.app_document_dir,
        "meta.json",
        AppMeta {
            schema_version: cx.schema_version,
            upgrading: false,
        },
    );
}

pub fn load_preference_data(cx: &BackendContext) -> PreferenceData {
    load_persistent_data::<PreferenceData>(&cx.app_document_dir, "preference.json")
        .unwrap_or_default()
}

pub fn save_preference_data(cx: &BackendContext, data: PreferenceData) {
    save_persistent_data(&cx.app_document_dir, "preference.json", data);
}

pub fn app_bootstrap(arg: ArgInitializeApp) -> BResult<BackendContext> {
    let cx = BackendContext {
        storage_path: arg.storage_path,
        app_document_dir: arg.app_document_dir,
        schema_version: arg.schema_version,
        server_port: Arc::new(Default::default()),
    };
    let port = start_server(&cx);
    cx.server_port
        .store(port, std::sync::atomic::Ordering::Relaxed);

    // Init
    init_persistent_state(&cx)?;
    Ok(cx)
}

fn init_persistent_state(cx: &BackendContext) -> BResult<()> {
    let _ = tracing::span!(Level::INFO, "init_persistent_state").enter();
    let meta = load_app_meta(&cx.app_document_dir);
    let prev_version = meta.schema_version;
    let curr_version = cx.schema_version;

    tracing::info!(
        "previous schema version {:?}, current schema version {:?}",
        prev_version,
        curr_version
    );

    if prev_version < curr_version {
        upgrade_db_schema(cx, prev_version)?;
        save_current_app_meta(cx);
    }
    Ok(())
}

fn upgrade_db_schema(cx: &BackendContext, prev_version: u32) -> BResult<()> {
    let conn = get_conn(cx)?;
    if prev_version < 1 {
        tracing::info!("start to upgrade to v1");
        conn.execute_batch(include_str!("../../../migrations/v1_init.sql"))?;
        tracing::info!("finish to upgrade to v1");
    }

    Ok(())
}
