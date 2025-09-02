use std::sync::{Arc, RwLock};

use bytes::Bytes;
use ease_client_schema::DataSourceKey;
use ease_remote_storage::{StorageBackendError, StorageBackendResult};

use crate::{error::BResult, services::get_asset_file, Backend};

#[uniffi::export]
pub async fn ct_get_asset(cx: Arc<Backend>, key: DataSourceKey) -> BResult<Option<Vec<u8>>> {
    let cx = cx.get_context();
    let file = get_asset_file(cx, key, 0).await?;
    let Some(file) = file else {
        return Ok(None);
    };

    let buf = file.bytes().await?;
    Ok(Some(buf.to_vec()))
}

#[derive(uniffi::Object)]
pub struct AssetStream {
    stream: async_channel::Receiver<StorageBackendResult<Bytes>>,
    size: Option<u64>,
}

#[uniffi::export]
impl AssetStream {
    pub async fn next(&self) -> BResult<Option<Vec<u8>>> {
        if let Ok(result) = self.stream.recv().await {
            let result = result?;
            Ok(Some(result.to_vec()))
        } else {
            Ok(None)
        }
    }

    pub fn size(&self) -> Option<u64> {
        self.size
    }
}

#[uniffi::export]
pub async fn ct_get_asset_stream(
    cx: Arc<Backend>,
    key: DataSourceKey,
    byte_offset: u64,
) -> BResult<Option<Arc<AssetStream>>> {
    let cx = cx.get_context();
    let file = get_asset_file(cx, key, byte_offset).await?;
    let Some(file) = file else {
        return Ok(None);
    };

    let len = file.size();
    let stream = file.into_rx();
    let stream = Arc::new(AssetStream {
        stream,
        size: len.map(|v| v as u64),
    });

    Ok(Some(stream))
}
