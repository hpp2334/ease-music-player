use std::sync::Arc;

use ease_client_shared::backends::app::ArgInitializeApp;

use crate::{ctx::BackendContext, error::BResult};

pub fn app_bootstrap(cx: &Arc<BackendContext>, arg: ArgInitializeApp) -> BResult<()> {
    static SCHEMA_VERSION: u32 = 1;

    cx.set_storage_path(&arg.storage_path);
    cx.set_app_document_dir(&arg.app_document_dir);
    cx.set_schema_version(SCHEMA_VERSION);
    // Init
    cx.database_server().init(arg.app_document_dir.clone());
    cx.database_server().save_schema_version(SCHEMA_VERSION)?;
    cx.asset_server().start(&cx, arg.app_document_dir);
    Ok(())
}
