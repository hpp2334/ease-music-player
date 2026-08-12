//! `ease-js-storage` — a [`StorageBackend`] whose `list` / `get` are served by a
//! JS storage-provider plugin over `ease-tur-rpc`.
//!
//! Each storage row that references a plugin provider instantiates one of
//! these. `list(dir)` is a control RPC returning a JSON array of entries;
//! `get(path, offset)` opens a streaming RPC and bridges the pushed chunks into
//! a [`StreamFile`] (channel-backed).
//!
//! JS handler contract (the provider registers these under
//! `"<provider_id>:list"` / `"<provider_id>:get"`):
//! - `list({ dir })` → `[{ name, path, size?, isDir }, ...]`
//! - `get({ streamId, path, offset })` → opens the stream at `offset`, pushes
//!   chunks via `pushChunk(streamId, bytes)`, ends via `endStream(streamId)` or
//!   `errorStream(streamId, msg)`, and returns `{ totalLength?, name?, contentType? }`.

use bytes::Bytes;
use ease_remote_storage::{Entry, StorageBackend, StorageBackendError, StorageBackendResult, StreamFile};
use ease_tur_rpc::{RpcClient, StreamChunk};
use futures_util::future::BoxFuture;
use serde::Deserialize;

/// A `StorageBackend` backed by a JS plugin via `ease-tur-rpc`.
///
/// `provider_id` selects the op namespace (`<provider>:list` / `<provider>:get`);
/// `instance` is the full `plugin_storage_id` (e.g. `onedrive:<uuid>`) carried
/// in every RPC's args so the JS plugin multiplexes between configured
/// instances (e.g. multiple OneDrive accounts).
#[derive(Clone)]
pub struct JsStorageBackend {
    rpc: RpcClient,
    provider_id: String,
    instance: String,
    handle: tokio::runtime::Handle,
}

impl JsStorageBackend {
    pub fn new(
        rpc: RpcClient,
        provider_id: impl Into<String>,
        instance: impl Into<String>,
        handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            rpc,
            provider_id: provider_id.into(),
            instance: instance.into(),
            handle,
        }
    }

    fn op(&self, suffix: &str) -> String {
        format!("{}:{}", self.provider_id, suffix)
    }
}

// JS-side entry shape (camelCase `isDir`).
#[derive(Deserialize)]
struct JsEntry {
    name: String,
    path: String,
    #[serde(default)]
    size: Option<usize>,
    #[serde(rename = "isDir", default)]
    is_dir: bool,
}

impl From<JsEntry> for Entry {
    fn from(j: JsEntry) -> Self {
        Entry {
            name: j.name,
            path: j.path,
            size: j.size,
            is_dir: j.is_dir,
        }
    }
}

#[derive(Deserialize)]
struct GetMeta {
    #[serde(rename = "totalLength", default)]
    total_length: Option<u64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "contentType", default)]
    content_type: Option<String>,
}

impl StorageBackend for JsStorageBackend {
    fn list(&self, dir: String) -> BoxFuture<StorageBackendResult<Vec<Entry>>> {
        let op = self.op("list");
        let instance = self.instance.clone();
        Box::pin(async move {
            let val = self
                .rpc
                .call(&op, serde_json::json!({ "instance": instance, "dir": dir }))
                .await
                .map_err(|e| StorageBackendError::Other(format!("list rpc: {e}")))?;
            let entries: Vec<JsEntry> = serde_json::from_value(val)
                .map_err(|e| StorageBackendError::Other(format!("list decode: {e}")))?;
            Ok(entries.into_iter().map(Entry::from).collect())
        })
    }

    fn get(&self, path: String, byte_offset: u64) -> BoxFuture<StorageBackendResult<StreamFile>> {
        let op = self.op("get");
        let handle = self.handle.clone();
        let instance = self.instance.clone();
        Box::pin(async move {
            let (meta, mut rx) = self
                .rpc
                .open_stream(
                    &op,
                    serde_json::json!({ "instance": instance, "path": path, "offset": byte_offset }),
                )
                .await
                .map_err(|e| StorageBackendError::Other(format!("get rpc: {e}")))?;
            let meta: GetMeta = serde_json::from_value(meta)
                .map_err(|e| StorageBackendError::Other(format!("get meta decode: {e}")))?;

            // Bridge the JS chunk stream into the async_channel the StreamFile
            // consumes. Spawned on the shared tokio runtime so it runs regardless
            // of which thread is reading the StreamFile (e.g. the cantode worker).
            let (atx, arx) = async_channel::bounded::<StorageBackendResult<Bytes>>(10);
            handle.spawn(async move {
                while let Some(chunk) = rx.recv().await {
                    match chunk {
                        StreamChunk::Data(b) => {
                            if atx.send(Ok(b)).await.is_err() {
                                break; // consumer dropped the StreamFile
                            }
                        }
                        StreamChunk::End => break,
                        StreamChunk::Error(msg) => {
                            let _ = atx
                                .send(Err(StorageBackendError::Other(format!(
                                    "js stream: {msg}"
                                ))))
                                .await;
                            break;
                        }
                    }
                }
                let _ = atx.close();
            });

            Ok(StreamFile::new_from_rx(
                arx,
                meta.total_length.map(|n| n as usize),
                byte_offset,
                meta.name.as_deref().unwrap_or(""),
                meta.content_type,
            ))
        })
    }
}
