use ease_remote_storage::set_global_local_storage_path;
use misty_vm::{
    client::{AsReadonlyMistyClientHandle, MistyClientHandle},
    states::MistyStateTrait,
    MistyState,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::Level;

use crate::modules::{
    error::EaseResult,
    playlist::service::initialize_all_playlist_state,
    preference::service::{init_preference_state, preference_state_to_data},
    server::service::spawn_server,
    storage::service::{init_local_storage_db_if_not_exist, update_storages_state},
    PreferenceState,
};

#[derive(Debug, Default, Clone, MistyState)]
pub struct GlobalAppState {
    pub storage_path: String,
    pub app_document_dir: String,
    pub schema_version: u32,
    pub has_storage_permission: bool,
}

pub struct ArgInitializeApp {
    pub app_document_dir: String,
    pub schema_version: u32,
    pub storage_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppMeta {
    pub schema_version: u32,
    pub upgrading: bool,
}

fn load_persistent_data<'a, T: Serialize + DeserializeOwned>(
    app: impl AsReadonlyMistyClientHandle<'a>,
    name: &str,
) -> Option<T> {
    let global_state = GlobalAppState::map(app, Clone::clone);
    let path = global_state.app_document_dir + name;

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
    app: impl AsReadonlyMistyClientHandle<'a>,
    name: &str,
    data: T,
) {
    let global_state = GlobalAppState::map(app, Clone::clone);
    let path = global_state.app_document_dir + name;

    let data = serde_json::to_string(&data).unwrap();
    std::fs::write(path, data).unwrap();
}

fn load_app_meta<'a>(app: impl AsReadonlyMistyClientHandle<'a>) -> AppMeta {
    let meta = load_persistent_data::<AppMeta>(app, "meta.json");
    match meta {
        Some(meta) => meta,
        None => AppMeta {
            schema_version: 0,
            upgrading: false,
        },
    }
}

fn save_current_app_meta<'a>(app: impl AsReadonlyMistyClientHandle<'a>) {
    let state = GlobalAppState::map(app, Clone::clone);
    save_persistent_data(
        app,
        "meta.json",
        AppMeta {
            schema_version: state.schema_version,
            upgrading: false,
        },
    );
}

pub fn load_preference_data<'a>(app: impl AsReadonlyMistyClientHandle<'a>) -> PreferenceState {
    load_persistent_data::<PreferenceState>(app, "preference.json").unwrap_or_default()
}

pub fn save_preference_data<'a>(app: impl AsReadonlyMistyClientHandle<'a>) {
    let data = preference_state_to_data(app);
    save_persistent_data(app, "preference.json", data);
}

pub fn get_db_conn_v2<'a>(
    app: impl AsReadonlyMistyClientHandle<'a>,
) -> EaseResult<ease_database::DbConnection> {
    let state = GlobalAppState::map(app, Clone::clone);
    let conn = ease_database::DbConnection::open(state.app_document_dir + "app.db")?;
    Ok(conn)
}

pub fn get_has_local_storage_permission(client: MistyClientHandle) -> bool {
    GlobalAppState::map(client, |state| state.has_storage_permission)
}

pub fn app_boostrap(client: MistyClientHandle, arg: ArgInitializeApp) -> EaseResult<()> {
    // Update global state
    GlobalAppState::update(client, |state| {
        state.app_document_dir = arg.app_document_dir;
        state.schema_version = arg.schema_version;
        state.storage_path = arg.storage_path;
    });

    // Init
    init_persistent_state(client)?;
    sync_storage_path(client);
    init_preference_state(client)?;
    spawn_server(client);

    // Load Data
    update_storages_state(client, true)?;
    initialize_all_playlist_state(client)?;
    Ok(())
}

pub fn update_storage_permission(client: MistyClientHandle, arg: bool) {
    GlobalAppState::update(client, |state| state.has_storage_permission = arg);
}

fn init_persistent_state(app: MistyClientHandle) -> EaseResult<()> {
    let _ = tracing::span!(Level::INFO, "init_persistent_state").enter();
    let state = GlobalAppState::map(app, Clone::clone);
    let meta = load_app_meta(app);
    let prev_version = meta.schema_version;
    let curr_version = state.schema_version;

    tracing::info!(
        "previous schema version {:?}, current schema version {:?}",
        prev_version,
        curr_version
    );

    if prev_version < curr_version {
        upgrade_db_schema(app, prev_version)?;
        save_current_app_meta(app);
    }
    init_local_storage_db_if_not_exist(app)?;
    Ok(())
}

fn sync_storage_path(client: MistyClientHandle) {
    let state = GlobalAppState::map(client, Clone::clone);
    set_global_local_storage_path(state.storage_path.to_string());
}

fn upgrade_db_schema(app: MistyClientHandle, prev_version: u32) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;
    if prev_version < 1 {
        tracing::info!("start to upgrade to v1");
        conn.execute_batch(include_str!("../../../migrations/v1_init.sql"))?;
        tracing::info!("finish to upgrade to v1");
    }

    Ok(())
}
