mod serve;

use std::{
    collections::HashMap,
    num::NonZero,
    sync::{atomic::AtomicU64, Arc, RwLock},
};

use ease_client_shared::backends::storage::DataSourceKey;
use ease_remote_storage::StreamFile;
use futures::StreamExt;
use lru::LruCache;
use misty_async::Task;
use serde::{Deserialize, Serialize};
use serve::{
    get_stream_file_by_loc, get_stream_file_by_music_id, get_stream_file_cover_by_music_id,
};

use crate::{ctx::BackendContext, error::BResult};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, uniffi::Enum)]
pub enum AssetLoadStatus {
    #[default]
    Pending,
    Loaded,
    NotFound,
    Error(String),
}

#[derive(PartialEq, Eq, Hash)]
struct AssetChunksCacheKey {
    key: DataSourceKey,
    byte_offset: u64,
}

struct AssetChunksSource {
    key: String,
    db: Arc<redb::Database>,
}

struct AssetChunksReader {
    init_remaining: u64,
    chunk_index: u64,
    chunks: Arc<AssetChunks>,
}

struct AssetChunks {
    chunk_index: AtomicU64,
    bytes: AtomicU64,
    source: AssetChunksSource,
}

#[derive(Debug, Serialize, Deserialize, uniffi::Enum)]
pub enum AssetChunkData {
    Status(AssetLoadStatus),
    #[serde(with = "serde_bytes")]
    Buffer(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize, uniffi::Enum)]
pub enum AssetChunkRead {
    NotOpen,
    None,
    Chunk(AssetChunkData),
}

struct OpenedAsset {
    _task: Option<Task<()>>,
    reader: Arc<RwLock<AssetChunksReader>>,
}

pub struct AssetServer {
    alloc: AtomicU64,
    opened_assets: RwLock<HashMap<u64, OpenedAsset>>,
    chunks_cache: RwLock<LruCache<AssetChunksCacheKey, Arc<AssetChunks>>>,
    db: RwLock<Option<Arc<redb::Database>>>,
}

pub struct AssetLoader {
    server: Arc<AssetServer>,
    cx: Arc<BackendContext>,
}

impl AssetChunksSource {
    fn def(key: &String) -> redb::TableDefinition<u64, Vec<u8>> {
        redb::TableDefinition::new(&key)
    }

    fn read_chunk(&self, index: u64) -> BResult<Option<AssetChunkData>> {
        let db = self.db.begin_read()?;
        let table_definition = Self::def(&self.key);
        let table = db.open_table(table_definition);
        if let Err(e) = &table {
            match e {
                redb::TableError::TableDoesNotExist(_) => {
                    return Ok(None);
                }
                _ => {}
            }
        }
        let table = table.unwrap();

        let data = table.get(index)?;

        if let Some(data) = data {
            let data = rmp_serde::from_read(std::io::Cursor::new(data.value())).unwrap();
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    fn add_chunk(&self, index: u64, chunk: AssetChunkData) -> BResult<()> {
        let w_txn = self.db.begin_write()?;
        let table_definition = Self::def(&self.key);
        {
            let mut table = w_txn.open_table(table_definition)?;
            let data = rmp_serde::to_vec(&chunk).unwrap();
            table.insert(index, data)?;
        }
        w_txn.commit()?;

        Ok(())
    }

    fn remove(&self) -> BResult<()> {
        let db = self.db.begin_write()?;
        let table_definition = Self::def(&self.key);
        db.delete_table(table_definition)?;
        Ok(())
    }
}

impl AssetChunksReader {
    fn read(&mut self) -> BResult<AssetChunkRead> {
        loop {
            let index = self.chunk_index;
            let chunk = self.chunks.source.read_chunk(index)?;

            if let Some(chunk) = chunk {
                self.chunk_index += 1;
                match chunk {
                    AssetChunkData::Status(asset_load_status) => {
                        return Ok(AssetChunkRead::Chunk(AssetChunkData::Status(
                            asset_load_status,
                        )));
                    }
                    AssetChunkData::Buffer(chunk) => {
                        let len = chunk.len() as u64;
                        let offset = len.min(self.init_remaining);
                        self.init_remaining -= offset;
                        if offset == 0 {
                            return Ok(AssetChunkRead::Chunk(AssetChunkData::Buffer(chunk)));
                        } else if offset < len {
                            return Ok(AssetChunkRead::Chunk(AssetChunkData::Buffer(
                                chunk[(offset as usize)..].to_vec(),
                            )));
                        }
                    }
                }
            } else {
                return Ok(AssetChunkRead::None);
            }
        }
    }
}

impl AssetChunks {
    fn new(source: AssetChunksSource) -> Arc<Self> {
        Arc::new(Self {
            chunk_index: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            source,
        })
    }

    fn push(&self, chunk: AssetChunkData) -> BResult<()> {
        let byte = match &chunk {
            AssetChunkData::Buffer(buf) => buf.len() as u64,
            _ => 0,
        };
        let index = self
            .chunk_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.source.add_chunk(index, chunk)?;
        self.bytes
            .fetch_add(byte, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn bytes(&self) -> u64 {
        self.bytes.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn reader(self: &Arc<Self>, byte_offset: u64) -> Arc<RwLock<AssetChunksReader>> {
        let reader = AssetChunksReader {
            init_remaining: byte_offset,
            chunk_index: 0,
            chunks: self.clone(),
        };
        Arc::new(RwLock::new(reader))
    }
}

impl Drop for AssetChunks {
    fn drop(&mut self) {
        self.source.remove().unwrap();
    }
}

impl AssetServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            alloc: Default::default(),
            opened_assets: Default::default(),
            chunks_cache: RwLock::new(LruCache::new(NonZero::new(6).unwrap())),
            db: Default::default(),
        })
    }

    pub(crate) fn init(&self, dir: String) {
        let p = dir + "chunk.redb";
        if std::fs::exists(p.as_str()).unwrap() {
            std::fs::remove_file(p.as_str()).unwrap();
        }

        let mut w = self.db.write().unwrap();
        let db = redb::Database::builder()
            .set_cache_size(100 << 20)
            .create(p)
            .expect("failed to init database");
        *w = Some(Arc::new(db));
    }

    pub async fn load(
        self: &Arc<Self>,
        cx: &Arc<BackendContext>,
        key: DataSourceKey,
        byte_offset: u64,
    ) -> BResult<Option<StreamFile>> {
        let cx = cx.clone();
        match key {
            DataSourceKey::Music { id } => get_stream_file_by_music_id(&cx, id, byte_offset).await,
            DataSourceKey::Cover { id } => {
                get_stream_file_cover_by_music_id(&cx, id, byte_offset).await
            }
            DataSourceKey::AnyEntry { entry } => {
                get_stream_file_by_loc(&cx, entry, byte_offset).await
            }
        }
    }

    async fn preload_impl(
        self: &Arc<Self>,
        cx: &Arc<BackendContext>,
        chunks: Arc<AssetChunks>,
        key: DataSourceKey,
        byte_offset: u64,
    ) -> BResult<()> {
        let file = self.load(&cx, key.clone(), byte_offset).await;

        if let Err(e) = &file {
            chunks.push(AssetChunkData::Status(AssetLoadStatus::Error(
                e.to_string(),
            )))?;
            return Ok(());
        }
        let file = file.unwrap();

        if file.is_none() {
            chunks.push(AssetChunkData::Status(AssetLoadStatus::NotFound))?;
            return Ok(());
        }

        let file = file.unwrap();
        let mut stream = Box::pin(file.into_stream());
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    chunks.push(AssetChunkData::Buffer(bytes.to_vec()))?;
                }
                Err(err) => {
                    chunks.push(AssetChunkData::Status(AssetLoadStatus::Error(
                        err.to_string(),
                    )))?;
                    return Ok(());
                }
            }
        }

        chunks.push(AssetChunkData::Status(AssetLoadStatus::Loaded))?;
        Ok(())
    }

    pub fn schedule_preload(
        self: &Arc<Self>,
        cx: &Arc<BackendContext>,
        key: DataSourceKey,
    ) -> BResult<()> {
        let (task, _) = self.trigger_preload(cx, key, 0);
        if let Some(task) = task {
            task.detach();
        }
        Ok(())
    }

    fn trigger_preload(
        self: &Arc<Self>,
        cx: &Arc<BackendContext>,
        key: DataSourceKey,
        byte_offset: u64,
    ) -> (Option<Task<()>>, Arc<AssetChunks>) {
        let source = AssetChunksSource {
            key: serde_json::to_string(&key).unwrap(),
            db: self.db.read().unwrap().clone().unwrap(),
        };
        let (existed, created) = {
            let key = AssetChunksCacheKey {
                key: key.clone(),
                byte_offset,
            };
            let mut w = self.chunks_cache.write().unwrap();
            if let Some(existed) = w.get(&key) {
                (Some(existed.clone()), None)
            } else {
                let created = w.get_or_insert(key, || AssetChunks::new(source)).clone();
                (None, Some(created))
            }
        };

        if let Some(chunks) = created {
            let this = self.clone();
            let task = {
                let chunks = chunks.clone();
                let cx = cx.clone();
                cx.async_runtime().clone().spawn(async move {
                    this.preload_impl(&cx, chunks, key, byte_offset)
                        .await
                        .unwrap();
                })
            };
            (Some(task), chunks)
        } else {
            (None, existed.unwrap())
        }
    }

    pub fn poll(self: &Arc<Self>, handle: u64) -> AssetChunkRead {
        let assets = self.opened_assets.read().unwrap();
        let asset = assets.get(&handle);
        if let Some(asset) = asset {
            let mut reader = asset.reader.write().unwrap();
            reader.read().unwrap()
        } else {
            AssetChunkRead::NotOpen
        }
    }

    pub fn open(
        self: &Arc<Self>,
        cx: &Arc<BackendContext>,
        key: DataSourceKey,
        byte_offset: u64,
    ) -> u64 {
        let cx = cx.clone();
        let id = self
            .alloc
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let to_reuse = {
            let mut w = self.chunks_cache.write().unwrap();
            let chunks = w.get(&AssetChunksCacheKey {
                key: key.clone(),
                byte_offset: 0,
            });
            if let Some(chunks) = chunks {
                if chunks.bytes() > byte_offset {
                    Some(chunks.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(chunks) = to_reuse {
            let mut w = self.opened_assets.write().unwrap();
            w.insert(
                id,
                OpenedAsset {
                    _task: None,
                    reader: chunks.reader(byte_offset),
                },
            );
            return id;
        }

        let (task, chunks) = self.trigger_preload(&cx, key, byte_offset);
        {
            let mut w = self.opened_assets.write().unwrap();
            w.insert(
                id,
                OpenedAsset {
                    _task: task,
                    reader: chunks.reader(0),
                },
            );
        }

        id
    }

    pub fn close(self: &Arc<Self>, handle: u64) {
        let mut w = self.opened_assets.write().unwrap();
        w.remove(&handle);
    }
}

impl AssetLoader {
    pub fn new(cx: Arc<BackendContext>, server: Arc<AssetServer>) -> Self {
        Self { cx, server }
    }

    pub async fn load(&self, key: DataSourceKey, byte_offset: u64) -> BResult<Option<StreamFile>> {
        self.server.load(&self.cx, key, byte_offset).await
    }

    pub fn poll(&self, handle: u64) -> AssetChunkRead {
        self.server.poll(handle)
    }

    pub fn open(&self, key: DataSourceKey, byte_offset: u64) -> u64 {
        self.server.open(&self.cx, key, byte_offset)
    }

    pub fn close(&self, id: u64) {
        self.server.close(id)
    }
}
