use ease_client_schema::{upgrade_v1_to_v2, upgrade_v2_to_v3, StorageType};

use crate::{ctx::BackendContext, error::BResult, objects::ArgUpsertStorage};

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArgInitializeApp {
    pub app_document_dir: String,
    pub app_cache_dir: String,
    pub storage_path: String,
}

pub fn app_bootstrap(cx: &BackendContext, arg: ArgInitializeApp) -> BResult<()> {
    tracing::info!("app bootstrap: {:?}", arg);
    cx.set_storage_path(&arg.storage_path);
    // Init
    init_database(cx, &arg)?;
    Ok(())
}

pub fn app_destroy(cx: &BackendContext) -> BResult<()> {
    cx.database_server().destroy();
    tracing::info!("app destroyed");
    Ok(())
}

fn init_database(cx: &BackendContext, arg: &ArgInitializeApp) -> BResult<()> {
    static SCHEMA_VERSION: u32 = 3;

    cx.database_server().init(arg.app_document_dir.clone());
    let old_schema_version = cx.database_server().get_schema_version()?;

    if old_schema_version < SCHEMA_VERSION {
        if old_schema_version < 1 {
            init_local_storage(cx)?;
            cx.database_server().save_schema_version(SCHEMA_VERSION)?;
        } else {
            if old_schema_version < 2 {
                upgrade_v1_to_v2(&cx.database_server().db())?;
            }
            if old_schema_version < 3 {
                upgrade_v2_to_v3(&cx.database_server().db())?;
            }
        }
    }

    let schema_version = cx.database_server().get_schema_version()?;
    assert_eq!(schema_version, SCHEMA_VERSION);
    tracing::info!(
        "old schema version is {}, now is {}",
        old_schema_version,
        SCHEMA_VERSION
    );

    Ok(())
}

fn init_local_storage(cx: &BackendContext) -> BResult<()> {
    cx.database_server().upsert_storage(ArgUpsertStorage {
        id: None,
        addr: Default::default(),
        alias: "Local".to_string(),
        username: Default::default(),
        password: Default::default(),
        is_anonymous: Default::default(),
        typ: StorageType::Local,
    })?;
    Ok(())
}
