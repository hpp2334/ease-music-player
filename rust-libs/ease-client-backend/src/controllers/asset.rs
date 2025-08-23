use std::sync::Arc;

use ease_client_schema::{DataSourceKey, StorageEntryLoc};
use ease_remote_storage::StorageBackendError;

use crate::{
    error::{BError, BResult},
    services::get_storage_backend,
    Backend,
};

#[uniffi::export]
pub async fn ct_get_asset(cx: Arc<Backend>, key: DataSourceKey) -> BResult<Option<Vec<u8>>> {
    let cx = cx.get_context();

    match key {
        DataSourceKey::Music { id } => todo!(),
        DataSourceKey::Cover { id } => todo!(),
        DataSourceKey::AnyEntry { entry } => {
            let storage_backend = get_storage_backend(cx, entry.storage_id)?;
            let Some(storage_backend) = storage_backend else {
                return Ok(None);
            };

            let file = storage_backend.get(entry.path, 0).await;
            if let Err(e) = &file {
                if e.is_not_found() {
                    return Ok(None);
                }
            }
            let file = file?;

            let buf = file.bytes().await?;
            Ok(Some(buf.to_vec()))
        }
    }
}
