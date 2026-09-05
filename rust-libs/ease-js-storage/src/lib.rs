//! `ease-js-storage` — a [`StorageBackend`] whose `list` / `get` are served by a
//! JS storage-provider plugin over `ease-tur-rpc`.
//!
//! Each storage row that references a plugin provider instantiates one of
//! these. `list(dir)` is a control RPC returning a JSON array of entries;
//! `get(path, offset)` opens a streaming RPC and bridges the credit-gated
//! chunks into a [`StreamFile`] (channel-backed).
//!
//! The ops are **contract literals, identical for every provider** — the host
//! never composes an op name. Identity rides the payload:
//! - `storage:list` `{ pluginId, storageId, dir }` →
//!   `[{ name, path, size?, isDir }, ...]` — a `hostRpc.registerHandler`.
//! - `storage:get` `{ pluginId, storageId, path, offset }` — a
//!   `hostRpc.registerStream` opener resolving
//!   `{ meta: { totalLength?, name?, contentType?, dataOffset? }, body,
//!   release?, mapError? }`. `meta` is replied up front, then `body` (an
//!   async iterable of `Uint8Array`) is pumped with host-granted credits.
//!   `dataOffset` (default: the requested `offset`) declares the byte offset
//!   the pushed chunks actually start at — a server that ignored the Range
//!   request reports `0` and the forwarding loop below drops the prefix
//!   bytes. `release` (conventionally `task.cancel()`) fires on every pump
//!   exit; `mapError` optionally marks mid-body errors.
//!
//! `pluginId` is the manifest id (`com.ease.webdav`); `storageId` is the
//! plugin-scoped instance (`webdav:<uuid>`, the storage row's
//! `plugin_storage_id` — the same value `ease.context.storageId$` publishes).
//! Both come straight from the registry row; the host derives nothing.
//!
//! Error semantics: opener errors / stream errors whose message starts
//! with `UNAUTHORIZED` or `TIMEOUT` are mapped to the corresponding
//! [`StorageBackendError`] variants so the UI can distinguish auth failures
//! and timeouts from other errors.

use bytes::Bytes;
use ease_remote_storage::{Entry, StorageBackend, StorageBackendError, StorageBackendResult, StreamFile};
use ease_tur_rpc::{RpcClient, StreamChunk};
use futures_util::future::BoxFuture;
use serde::Deserialize;

/// Classify a JS-side error message into the typed variants the host UI
/// reacts to (`is_unauthorized` / `is_timeout`).
fn classify_error(message: String) -> StorageBackendError {
    let trimmed = message.trim_start();
    if let Some(rest) = trimmed.strip_prefix("UNAUTHORIZED") {
        StorageBackendError::Unauthorized(rest.trim_start_matches([':', ' ']).to_string())
    } else if let Some(rest) = trimmed.strip_prefix("TIMEOUT") {
        StorageBackendError::Timeout(rest.trim_start_matches([':', ' ']).to_string())
    } else {
        StorageBackendError::Other(message)
    }
}

/// Map an RPC failure: a JS handler error keeps its (classified) message;
/// transport errors stay opaque.
fn map_rpc_error(context: &str, e: ease_tur_rpc::RpcError) -> StorageBackendError {
    match e {
        ease_tur_rpc::RpcError::Handler(msg) => classify_error(msg),
        other => StorageBackendError::Other(format!("{context} rpc: {other}")),
    }
}

/// A `StorageBackend` backed by a JS plugin via `ease-tur-rpc`.
///
/// `plugin_id` + `storage_id` come straight from the storage registry row
/// (`plugin_id` = manifest id; `storage_id` = the `plugin_storage_id`
/// instance, e.g. `onedrive:<uuid>`) and ride every RPC payload verbatim so
/// the JS plugin multiplexes between configured instances (e.g. multiple
/// OneDrive accounts).
#[derive(Clone)]
pub struct JsStorageBackend {
    rpc: RpcClient,
    plugin_id: String,
    storage_id: String,
    handle: tokio::runtime::Handle,
}

impl JsStorageBackend {
    pub fn new(
        rpc: RpcClient,
        plugin_id: impl Into<String>,
        storage_id: impl Into<String>,
        handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            rpc,
            plugin_id: plugin_id.into(),
            storage_id: storage_id.into(),
            handle,
        }
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
    /// Byte offset the pushed chunks actually start at. Defaults to the
    /// requested offset; a server that ignored the Range request reports 0
    /// and the prefix bytes are dropped below.
    #[serde(rename = "dataOffset", default)]
    data_offset: Option<u64>,
}

impl StorageBackend for JsStorageBackend {
    fn list(&self, dir: String) -> BoxFuture<StorageBackendResult<Vec<Entry>>> {
        let plugin_id = self.plugin_id.clone();
        let storage_id = self.storage_id.clone();
        Box::pin(async move {
            let val = self
                .rpc
                .call_host(
                    "storage:list",
                    serde_json::json!({ "pluginId": plugin_id, "storageId": storage_id, "dir": dir }),
                )
                .await
                .map_err(|e| map_rpc_error("list", e))?;
            let entries: Vec<JsEntry> = serde_json::from_value(val)
                .map_err(|e| StorageBackendError::Other(format!("list decode: {e}")))?;
            Ok(entries.into_iter().map(Entry::from).collect())
        })
    }

    fn get(&self, path: String, byte_offset: u64) -> BoxFuture<StorageBackendResult<StreamFile>> {
        let handle = self.handle.clone();
        let plugin_id = self.plugin_id.clone();
        let storage_id = self.storage_id.clone();
        Box::pin(async move {
            let (meta, mut rx) = self
                .rpc
                .open_stream(
                    "storage:get",
                    serde_json::json!({
                        "pluginId": plugin_id,
                        "storageId": storage_id,
                        "path": path,
                        "offset": byte_offset,
                    }),
                )
                .await
                .map_err(|e| map_rpc_error("get", e))?;
            let meta: GetMeta = serde_json::from_value(meta)
                .map_err(|e| StorageBackendError::Other(format!("get meta decode: {e}")))?;
            let data_offset = meta.data_offset.unwrap_or(byte_offset);
            if data_offset > byte_offset {
                return Err(StorageBackendError::Other(format!(
                    "get meta decode: dataOffset {data_offset} beyond requested offset {byte_offset}"
                )));
            }
            // Prefix bytes to drop before real data starts (ignored Range).
            let mut skip = byte_offset - data_offset;

            // Bridge the JS chunk stream into the async_channel the StreamFile
            // consumes. Spawned on the shared tokio runtime so it runs regardless
            // of which thread is reading the StreamFile (e.g. the cantode worker).
            let (atx, arx) = async_channel::bounded::<StorageBackendResult<Bytes>>(10);
            handle.spawn(async move {
                while let Some(chunk) = rx.recv().await {
                    match chunk {
                        StreamChunk::Data(b) => {
                            let b = if skip > 0 {
                                if b.len() as u64 <= skip {
                                    skip -= b.len() as u64;
                                    continue;
                                }
                                let b = b.slice(skip as usize..);
                                skip = 0;
                                b
                            } else {
                                b
                            };
                            if atx.send(Ok(b)).await.is_err() {
                                break; // consumer dropped the StreamFile
                            }
                        }
                        StreamChunk::End => break,
                        StreamChunk::Error(msg) => {
                            let _ = atx
                                .send(Err(classify_error(msg)))
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
