use ease_client_schema::StorageType;

use crate::{
    ctx::BackendContext,
    error::BResult,
    objects::ArgUpsertStorage,
    repositories::app::SCHEMA_VERSION,
};

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArgInitializeApp {
    pub app_document_dir: String,
    pub app_cache_dir: String,
    pub storage_path: String,
}

pub async fn app_bootstrap(cx: &BackendContext, arg: ArgInitializeApp) -> BResult<()> {
    tracing::info!("app bootstrap: {:?}", arg);
    cx.set_storage_path(&arg.storage_path);
    init_database(cx, &arg).await?;
    Ok(())
}

pub async fn app_destroy(cx: &BackendContext) -> BResult<()> {
    cx.database_server().destroy();
    tracing::info!("app destroyed");
    Ok(())
}

async fn init_database(cx: &BackendContext, arg: &ArgInitializeApp) -> BResult<()> {
    // `DatabaseServer::init` runs ease_client_migration::migrate, which:
    //   1. Opens/creates data.db.
    //   2. Runs sea-orm-migration to the latest schema (v4).
    //   3. If a legacy data.redb exists at <doc_dir>/data.redb, imports all
    //      rows into SQLite and deletes data.redb.
    cx.database_server().init(arg.app_document_dir.clone()).await?;

    let old_schema_version = cx.database_server().get_schema_version().await?;
    if old_schema_version == 0 {
        // Fresh install with no legacy redb — seed local storage.
        init_local_storage(cx).await?;
        cx.database_server().save_schema_version(SCHEMA_VERSION).await?;
    }

    let schema_version = cx.database_server().get_schema_version().await?;
    tracing::info!(
        "old schema version was {}, now is {}",
        old_schema_version,
        schema_version
    );

    Ok(())
}

async fn init_local_storage(cx: &BackendContext) -> BResult<()> {
    cx.database_server()
        .upsert_storage(ArgUpsertStorage {
            id: None,
            addr: Default::default(),
            alias: "Local".to_string(),
            username: Default::default(),
            password: Default::default(),
            is_anonymous: Default::default(),
            typ: StorageType::Local,
        })
        .await?;
    Ok(())
}
