//! WebDAV-only storage controllers: create/update (`ct_upsert_webdav_storage`)
//! and connection test (`ct_test_storage`). OneDrive is no longer a core
//! storage kind (it is a JS plugin provider), and Local is not user-createable,
//! so the only createable/editable kind is WebDAV.

use std::sync::Arc;
use ease_client_tokio::tokio_runtime;

use crate::{
    error::BResult,
    objects::StorageConnectionTestResult,
    services::{build_webdav_backend, upsert_webdav_storage},
    ArgUpsertWebdavStorage, Backend,
};

pub async fn ct_upsert_webdav_storage(
    cx: Arc<Backend>,
    arg: ArgUpsertWebdavStorage,
) -> BResult<()> {
    tokio_runtime().handle().spawn(async move {
        let cx = cx.get_context();
        upsert_webdav_storage(cx, arg).await?;
        Ok(())
    }).await.unwrap()
}

pub async fn ct_test_webdav_storage(
    cx: Arc<Backend>,
    arg: ArgUpsertWebdavStorage,
) -> BResult<StorageConnectionTestResult> {
    tokio_runtime().handle().spawn(async move {
        let _cx = cx.get_context();
        let backend = build_webdav_backend(arg.addr, arg.username, arg.password, arg.is_anonymous);
        let res = backend.list("/".to_string()).await;

        match res {
            Ok(_) => Ok(StorageConnectionTestResult::Success),
            Err(e) => {
                tracing::warn!("ct_test_storage, {e:?}");
                if e.is_unauthorized() {
                    Ok(StorageConnectionTestResult::Unauthorized)
                } else if e.is_timeout() {
                    Ok(StorageConnectionTestResult::Timeout)
                } else {
                    Ok(StorageConnectionTestResult::OtherError)
                }
            }
        }
    }).await.unwrap()
}
