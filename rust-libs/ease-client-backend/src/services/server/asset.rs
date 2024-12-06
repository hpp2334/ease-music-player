use std::{
    net::SocketAddr,
    sync::{atomic::AtomicU16, Arc, RwLock},
};

use async_stream::stream;
use axum::{
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use ease_client_shared::backends::{music::MusicId, storage::DataSourceKey};
use ease_remote_storage::{StatusCode, StreamFile};
use futures::StreamExt;
use misty_async::Task;

use crate::{
    ctx::{BackendContext, WeakBackendContext},
    error::BResult,
};

use super::{
    chunks::{
        AssetChunkData, AssetChunks, AssetChunksCacheKey, AssetChunksManager, AssetChunksReader,
        AssetLoadStatus,
    },
    load_asset,
};

pub struct AssetServer {
    port: AtomicU16,
    server_handle: RwLock<Option<Task<()>>>,
    manager: AssetChunksManager,
}

fn parse_request_range(headers: &HeaderMap) -> Option<u64> {
    let val = headers.get(header::RANGE);
    if let Some(val) = val {
        let val = val.to_str();
        if let Ok(val) = val {
            const BYTES_PREFIX: &str = "bytes=";
            if val.starts_with(BYTES_PREFIX) {
                let range_value = &val[BYTES_PREFIX.len()..];
                let start = range_value.split('-').next().ok_or("Invalid range format");
                if let Ok(start) = start {
                    let start = start.parse::<u64>().map_err(|_| "Invalid byte offset");
                    if let Ok(start) = start {
                        return Some(start);
                    }
                }
            }
        }
    }
    return None;
}

fn reader_into_stream(
    reader: Arc<AssetChunksReader>,
) -> impl futures_util::Stream<Item = BResult<Bytes>> {
    stream! {
        loop {
            let read = reader.read().await;
            match read {
                Ok(buf) => {
                    match buf {
                        Some(buf) => {
                            yield(Ok(Bytes::from(buf)))
                        },
                        None => {
                            break;
                        },
                    }
                },
                Err(e) => {
                    yield(Err(e));
                    break;
                },
            }
        }
    }
}

#[axum::debug_handler]
async fn handle_music_download(
    headers: HeaderMap,
    axum::extract::State(cx): axum::extract::State<WeakBackendContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> axum::response::Response {
    let id = MusicId::wrap(id);
    tracing::info!("handle_music_download {:?}", id);

    if let Some(cx) = cx.upgrade() {
        cx.asset_server()
            .handle_got_stream_file(&cx, headers, id)
            .await
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[axum::debug_handler]
async fn handle_music_meta(
    axum::extract::State(cx): axum::extract::State<WeakBackendContext>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> axum::response::Response {
    let id = MusicId::wrap(id);

    if let Some(cx) = cx.upgrade() {
        cx.asset_server().handle_got_stream_meta(&cx, id).await
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

impl Drop for AssetServer {
    fn drop(&mut self) {
        self.server_handle.write().unwrap().take();
    }
}

impl AssetServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            port: Default::default(),
            server_handle: Default::default(),
            manager: AssetChunksManager::new(3),
        })
    }

    pub fn serve_music_url(&self, id: MusicId) -> String {
        let port = self.port.load(std::sync::atomic::Ordering::Relaxed);
        let id = *id.as_ref();
        format!("http://127.0.0.1:{}/music/{}", port, id)
    }

    pub fn serve_music_meta_url(&self, id: MusicId) -> String {
        let port = self.port.load(std::sync::atomic::Ordering::Relaxed);
        let id = *id.as_ref();
        format!("http://127.0.0.1:{}/music-meta/{}", port, id)
    }

    pub fn start(&self, cx: &BackendContext, dir: String) {
        self.start_db(cx, dir);
        self.start_server(cx);
    }

    pub fn schedule_preload(self: &Arc<Self>, cx: &BackendContext, id: MusicId) -> BResult<()> {
        let this = self.clone();
        let rt = cx.async_runtime();
        let cx = cx.weak();
        rt.clone()
            .spawn(async move {
                if let Some(cx) = cx.upgrade() {
                    let _ = this.build_chunks_reader(&cx, id, 0).await;
                }
            })
            .detach();
        Ok(())
    }

    pub fn evict_cache(&self, id: MusicId) {
        self.manager.evict(id, 0);
    }

    fn start_db(&self, _cx: &BackendContext, dir: String) {
        let p = dir + "asset_cache";
        if std::fs::exists(p.as_str()).unwrap() {
            std::fs::remove_dir_all(p.as_str()).unwrap();
        }
        self.manager.init(p);
    }

    fn start_server(&self, cx: &BackendContext) {
        let router_svc = axum::Router::new()
            .route("/music/:id", axum::routing::get(handle_music_download))
            .route("/music-meta/:id", axum::routing::get(handle_music_meta))
            .with_state(cx.weak())
            .into_make_service();

        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let incomming = axum::Server::bind(&addr)
            .http1_max_buf_size(20_000_000) // ~20MB
            .serve(router_svc);

        let port = incomming.local_addr().port();

        let task = cx.async_runtime().spawn(async move {
            let _ = incomming.await.unwrap();
        });
        {
            let mut w = self.server_handle.write().unwrap();
            *w = Some(task);
        }

        tracing::info!("setup a local server on {}", port);
        self.port.store(port, std::sync::atomic::Ordering::Relaxed);
    }

    async fn handle_got_stream_file(
        self: &Arc<AssetServer>,
        cx: &BackendContext,
        headers: HeaderMap,
        id: MusicId,
    ) -> Response {
        let (byte_offset, is_partial) = {
            let byte_offset = parse_request_range(&headers);

            if let Some(byte_offset) = byte_offset {
                (byte_offset, true)
            } else {
                (0, false)
            }
        };

        let status = if is_partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };

        let reader = self.build_chunks_reader(cx, id, byte_offset).await.unwrap();
        let all_bytes = reader.all_bytes();

        let mut headers = HeaderMap::new();
        headers.append(
            header::CONTENT_TYPE,
            HeaderValue::from_str("application/octet-stream").unwrap(),
        );
        if is_partial {
            let val = if all_bytes == 0 {
                format!("bytes {}-/*", byte_offset)
            } else {
                format!(
                    "bytes {}-{}/{}",
                    byte_offset,
                    byte_offset + all_bytes - 1,
                    byte_offset + all_bytes
                )
            };
            headers.append(
                header::CONTENT_RANGE,
                HeaderValue::from_str(val.as_str()).unwrap(),
            );
        } else if all_bytes > 0 {
            let val = all_bytes.to_string();
            headers.append(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(val.as_str()).unwrap(),
            );
        }

        let stream = reader_into_stream(reader);

        let body = axum::body::StreamBody::new(stream);
        return (status, headers, body).into_response();
    }

    async fn handle_got_stream_meta(
        self: &Arc<AssetServer>,
        cx: &BackendContext,
        id: MusicId,
    ) -> Response {
        let file = load_asset(&cx, DataSourceKey::Music { id }, 0).await;
        if file.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let file = file.unwrap();
        if file.is_none() {
            return StatusCode::NOT_FOUND.into_response();
        }
        let file = file.unwrap();

        let mut headers = HeaderMap::new();
        headers.append(
            header::CONTENT_TYPE,
            HeaderValue::from_str("application/octet-stream").unwrap(),
        );
        if let Some(size) = file.size() {
            headers.append(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(size.to_string().as_str()).unwrap(),
            );
        }
        let body = axum::body::StreamBody::new(file.into_stream());
        return (headers, body).into_response();
    }

    async fn build_chunks_reader(
        self: &Arc<Self>,
        cx: &BackendContext,
        id: MusicId,
        byte_offset: u64,
    ) -> BResult<Arc<AssetChunksReader>> {
        if byte_offset > 0 {
            let from_zero_chunks = self.build_chunks_impl(cx, id, 0).await?;
            if from_zero_chunks.read().unwrap().current_bytes() >= byte_offset {
                return Ok(AssetChunksReader::new(&from_zero_chunks, byte_offset));
            }
        }

        let chunks = self.build_chunks_impl(cx, id, byte_offset).await?;
        return Ok(AssetChunksReader::new(&chunks, 0));
    }

    async fn build_chunks_impl(
        self: &Arc<Self>,
        cx: &BackendContext,
        id: MusicId,
        byte_offset: u64,
    ) -> BResult<Arc<RwLock<AssetChunks>>> {
        let key = DataSourceKey::Music { id };
        let cached_key = AssetChunksCacheKey { id, byte_offset };

        let (chunks, existed) = self.manager.get_or_create(id, byte_offset);

        tracing::info!(
            "build_chunks cached_key = {:?}, existed = {:?}",
            cached_key,
            existed
        );

        if !existed {
            let file = load_asset(&cx, key.clone(), byte_offset).await;
            match file {
                Ok(file) => match file {
                    Some(file) => {
                        let chunks = chunks.clone();
                        if let Some(size) = file.size() {
                            chunks.write().unwrap().set_all_bytes(size as u64);
                        }

                        let this = self.clone();
                        let task = {
                            let chunks = chunks.clone();
                            cx.async_runtime().clone().spawn(async move {
                                this.preload_impl(chunks, file).await.unwrap();
                            })
                        };
                        task.detach();
                    }
                    None => {
                        chunks
                            .write()
                            .unwrap()
                            .push(AssetChunkData::Status(AssetLoadStatus::NotFound))?;
                    }
                },
                Err(e) => {
                    chunks.write().unwrap().push(AssetChunkData::Status(
                        AssetLoadStatus::Error(e.to_string()),
                    ))?;
                }
            }
        }
        return Ok(chunks.clone());
    }

    async fn preload_impl(
        self: &Arc<Self>,
        chunks: Arc<RwLock<AssetChunks>>,
        file: StreamFile,
    ) -> BResult<()> {
        let mut stream = Box::pin(file.into_stream());
        let mut buffer_cache: Vec<u8> = Default::default();

        let flush = |buffer_cache: &mut Vec<u8>| -> BResult<()> {
            if !buffer_cache.is_empty() {
                chunks
                    .write()
                    .unwrap()
                    .push(AssetChunkData::Buffer(buffer_cache.clone()))?;
                buffer_cache.clear();
            }
            Ok(())
        };
        let write = |bytes: Bytes, buffer_cache: &mut Vec<u8>| -> BResult<()> {
            buffer_cache.extend(bytes);

            if buffer_cache.len() >= (16 << 20) {
                flush(buffer_cache)?;
            }
            Ok(())
        };

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    write(bytes, &mut buffer_cache)?;
                }
                Err(err) => {
                    flush(&mut buffer_cache)?;
                    chunks.write().unwrap().push(AssetChunkData::Status(
                        AssetLoadStatus::Error(err.to_string()),
                    ))?;
                    return Ok(());
                }
            }
        }

        flush(&mut buffer_cache)?;

        chunks
            .write()
            .unwrap()
            .push(AssetChunkData::Status(AssetLoadStatus::Loaded))?;
        Ok(())
    }
}
