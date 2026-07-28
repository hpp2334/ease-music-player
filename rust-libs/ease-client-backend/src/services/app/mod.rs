use serde::{Deserialize, Serialize};

use crate::{ctx::BackendContext, error::BResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    //
    // Local storage is NOT seeded here. The biz layer synthesizes it on every
    // `list_storage` call (see services::storage::list_storage), so it is
    // always present regardless of DB / migration state.
    cx.database_server().init(arg.app_document_dir.clone()).await?;
    let schema_version = cx.database_server().get_schema_version().await?;
    tracing::info!("database initialized; schema version = {}", schema_version);
    Ok(())
}
