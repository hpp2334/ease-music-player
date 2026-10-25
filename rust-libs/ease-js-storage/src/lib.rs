//! `ease-js-storage` — a [`StorageBackend`] whose `list` / `get` are served
//! by a rut storage-provider plugin over `ease-tur-rpc`.
//!
//! Each storage row that references a plugin provider instantiates one of
//! these. `list(dir)` is a control RPC returning a JSON array of entries;
//! `get(path, offset)` is a **chunked pull**: an opener RPC returns the
//! metadata + a session id, then the host pulls fixed-size chunks until
//! `eof`, then releases the session. Pull = natural backpressure (the
//! consumer drains the bounded channel; the loop only pulls the next chunk
//! after the previous one was handed off) — the old credit-gated push
//! protocol is gone.
//!
//! The ops are **contract literals, identical for every provider** — the
//! host never composes an op name. Identity rides the payload:
//! - `storage:list` `{ pluginId, storageId, dir }` →
//!   `[{ name, path, size?, isDir, createdAt?, modifiedAt? }, ...]`
//!   (a backend op entry fn answering the result array).
//! - `storage:get` `{ pluginId, storageId, path, offset }` →
//!   `{ sid, meta: { totalLength?, name?, contentType?, dataOffset? } }`.
//!   `dataOffset` (default: the requested `offset`) declares the byte
//!   offset the chunks actually start at — a server that ignored the Range
//!   request reports `0` and the forwarding loop below drops the prefix.
//! - `storage:get-chunk` `{ sid, offset, len }` →
//!   `{ data: <base64>, eof: bool }` — `data` empty with `eof` set ends the
//!   stream.
//! - `storage:release` `{ sid }` — drop the plugin-side source (called on
//!   eof and on consumer cancellation; best-effort).
//!
//! The plugin backend keeps its per-session source state in its own module
//! cells, keyed by `sid`.
//!
//! `pluginId` is the manifest id (`com.ease.webdav`); `storageId` is the
//! plugin-scoped instance (`webdav:<uuid>`, the storage row's
//! `plugin_storage_id`). Both come straight from the registry row; the host
//! derives nothing.
//!
//! Error semantics: handler errors whose message starts with `UNAUTHORIZED`
//! or `TIMEOUT` are mapped to the corresponding [`StorageBackendError`]
//! variants so the UI can distinguish auth failures and timeouts from
//! other errors.

use base64::Engine as _;
use bytes::Bytes;
use ease_remote_storage::{Entry, StorageBackend, StorageBackendError, StorageBackendResult, StreamFile};
use ease_tur_rpc::RpcClient;
use futures_util::future::BoxFuture;
use serde::Deserialize;

/// Classify a plugin-side error message into the typed variants the host UI
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

/// Map an RPC failure: a handler error keeps its (classified) message;
/// transport errors stay opaque.
fn map_rpc_error(context: &str, e: ease_tur_rpc::RpcError) -> StorageBackendError {
    match e {
        ease_tur_rpc::RpcError::Handler(msg) => classify_error(msg),
        other => StorageBackendError::Other(format!("{context} rpc: {other}")),
    }
}

/// A `StorageBackend` backed by a rut plugin via `ease-tur-rpc`.
///
/// `plugin_id` + `storage_id` come straight from the storage registry row
/// (`plugin_id` = manifest id; `storage_id` = the `plugin_storage_id`
/// instance, e.g. `onedrive:<uuid>`) and ride every RPC payload verbatim so
/// the plugin multiplexes between configured instances (e.g. multiple
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

    fn args(&self) -> serde_json::Value {
        serde_json::json!({
            "pluginId": self.plugin_id,
            "storageId": self.storage_id,
        })
    }
}

// Plugin-side entry shape (camelCase `isDir`).
#[derive(Deserialize)]
struct JsEntry {
    name: String,
    path: String,
    #[serde(default)]
    size: Option<usize>,
    #[serde(rename = "isDir", default)]
    is_dir: bool,
    #[serde(rename = "createdAt", default)]
    created_at: Option<u64>,
    #[serde(rename = "modifiedAt", default)]
    modified_at: Option<u64>,
}

impl From<JsEntry> for Entry {
    fn from(j: JsEntry) -> Self {
        Entry {
            name: j.name,
            path: j.path,
            size: j.size,
            is_dir: j.is_dir,
            created_at: j.created_at,
            modified_at: j.modified_at,
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
    /// Byte offset the pulled chunks actually start at. Defaults to the
    /// requested offset; a server that ignored the Range request reports 0
    /// and the prefix bytes are dropped below.
    #[serde(rename = "dataOffset", default)]
    data_offset: Option<u64>,
}

#[derive(Deserialize)]
struct OpenReply {
    sid: u64,
    #[serde(default)]
    meta: Option<GetMeta>,
}

#[derive(Deserialize)]
struct ChunkReply {
    #[serde(default)]
    data: String,
    #[serde(default)]
    eof: bool,
}

/// Chunk size requested per `storage:get-chunk` pull. Sized for the
/// `BufferedSource` read-ahead: big enough that the cross-thread RPC
/// round-trips amortize, small enough that a chunk lands well inside the
/// source's window.
const CHUNK_LEN: u64 = 256 * 1024;

impl StorageBackend for JsStorageBackend {
    fn list(&self, dir: String) -> BoxFuture<StorageBackendResult<Vec<Entry>>> {
        let rpc = self.rpc.clone();
        let mut args = self.args();
        args["dir"] = serde_json::Value::String(dir);
        Box::pin(async move {
            let val = rpc
                .call_host("storage:list", args)
                .await
                .map_err(|e| map_rpc_error("list", e))?;
            let entries: Vec<JsEntry> = serde_json::from_value(val)
                .map_err(|e| StorageBackendError::Other(format!("list decode: {e}")))?;
            Ok(entries.into_iter().map(Entry::from).collect())
        })
    }

    fn get(&self, path: String, byte_offset: u64) -> BoxFuture<StorageBackendResult<StreamFile>> {
        let handle = self.handle.clone();
        let rpc = self.rpc.clone();
        let mut args = self.args();
        args["path"] = serde_json::Value::String(path);
        args["offset"] = serde_json::json!(byte_offset);
        Box::pin(async move {
            // ---- opener: metadata + session id ----------------------------
            let open = rpc
                .call_host("storage:get", args)
                .await
                .map_err(|e| map_rpc_error("get", e))?;
            let open: OpenReply = serde_json::from_value(open)
                .map_err(|e| StorageBackendError::Other(format!("get open decode: {e}")))?;
            let meta: GetMeta = open
                .meta
                .ok_or_else(|| StorageBackendError::Other("get open decode: missing meta".into()))?;
            let data_offset = meta.data_offset.unwrap_or(byte_offset);
            if data_offset > byte_offset {
                return Err(StorageBackendError::Other(format!(
                    "get meta decode: dataOffset {data_offset} beyond requested offset {byte_offset}"
                )));
            }
            // Prefix bytes to drop before real data starts (ignored Range).
            let mut skip = byte_offset - data_offset;
            let sid = open.sid;

            // ---- the chunked pull ------------------------------------------
            // Bridges the pull loop into the async_channel the StreamFile
            // consumes. Spawned on the shared tokio runtime so it runs
            // regardless of which thread is reading the StreamFile (e.g. the
            // cantode worker). Pull-driven: the next chunk is only fetched
            // after the previous one was handed off (or dropped).
            let (atx, arx) = async_channel::bounded::<StorageBackendResult<Bytes>>(10);
            handle.spawn(async move {
                let mut offset = data_offset;
                let result = loop {
                    let mut chunk_args = serde_json::json!({
                        "sid": sid,
                        "offset": offset,
                        "len": CHUNK_LEN,
                    });
                    let _ = &mut chunk_args;
                    let reply = match rpc.call_host("storage:get-chunk", chunk_args).await {
                        Ok(v) => v,
                        Err(ease_tur_rpc::RpcError::Handler(msg)) => {
                            break Err(classify_error(msg));
                        }
                        Err(e) => {
                            break Err(StorageBackendError::Other(format!("get rpc: {e}")));
                        }
                    };
                    let chunk: ChunkReply = match serde_json::from_value(reply) {
                        Ok(c) => c,
                        Err(e) => {
                            break Err(StorageBackendError::Other(format!(
                                "get chunk decode: {e}"
                            )));
                        }
                    };
                    // Consume the data BEFORE honoring eof — the final chunk
                    // carries both.
                    let mut data = match base64::engine::general_purpose::STANDARD
                        .decode(chunk.data.as_bytes())
                    {
                        Ok(d) => d,
                        Err(e) => {
                            break Err(StorageBackendError::Other(format!(
                                "get chunk base64: {e}"
                            )));
                        }
                    };
                    if data.is_empty() && chunk.eof {
                        break Ok(());
                    }
                    let n = data.len() as u64;
                    offset += n;
                    if skip > 0 {
                        if n <= skip {
                            skip -= n;
                            if chunk.eof {
                                break Ok(());
                            }
                            continue;
                        }
                        data.drain(..skip as usize);
                        skip = 0;
                    }
                    if !data.is_empty() && atx.send(Ok(Bytes::from(data))).await.is_err() {
                        break Ok(()); // consumer dropped the StreamFile
                    }
                    if chunk.eof {
                        break Ok(());
                    }
                };
                // Release the plugin-side source (eof or cancel — best-effort).
                let _ = rpc
                    .call_host(
                        "storage:release",
                        serde_json::json!({ "sid": sid }),
                    )
                    .await;
                if let Err(e) = result {
                    let _ = atx.send(Err(e)).await;
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
