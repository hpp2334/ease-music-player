use ease_client_shared::backends::{
    app::ArgInitializeApp,
    storage::{ArgUpsertStorage, StorageType},
};

use crate::{ctx::BackendContext, error::BResult};

pub fn app_bootstrap(cx: &BackendContext, arg: ArgInitializeApp) -> BResult<()> {
    cx.set_storage_path(&arg.storage_path);
    // Init
    init_database(cx, &arg)?;
    cx.asset_server().start(&cx, arg.app_document_dir);
    Ok(())
}

fn init_database(cx: &BackendContext, arg: &ArgInitializeApp) -> BResult<()> {
    static SCHEMA_VERSION: u32 = 2;

    cx.database_server().init(arg.app_document_dir.clone());
    let old_schema_version = cx.database_server().get_schema_version()?;
    tracing::info!("old schema version is {}, now is {}", old_schema_version, SCHEMA_VERSION);

    if old_schema_version < SCHEMA_VERSION {
        if old_schema_version < 1 {
            init_local_storage(cx)?;
        } else if old_schema_version < 2 {
            cx.database_server().cleanup_invalid_storage_music_entries()?;
        }
    }

    cx.database_server().save_schema_version(SCHEMA_VERSION)?;
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
