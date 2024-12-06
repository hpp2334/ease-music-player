use std::{
    num::NonZero,
    sync::{
        atomic::{AtomicU64, AtomicUsize},
        Arc, RwLock, Weak,
    },
};

use ease_client_shared::backends::{music::MusicId, storage::BlobId};
use lru::LruCache;
use serde::{Deserialize, Serialize};

use crate::{
    error::{BError, BResult},
    repositories::blob::BlobManager,
};

#[derive(Debug, bitcode::Encode, bitcode::Decode, PartialEq, Eq, uniffi::Enum, Clone)]
pub enum AssetLoadStatus {
    NotFound,
    Loaded,
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct AssetChunksCacheKey {
    pub id: MusicId,
    pub byte_offset: u64,
}

#[derive(Debug)]
pub enum AssetChunkData {
    Status(AssetLoadStatus),
    Buffer(Vec<u8>),
}

#[derive(Debug, Clone)]
enum AssetChunkDataInternal {
    Status(AssetLoadStatus),
    Buffer(BlobId),
}

struct AssetChunksSource {
    blob: Arc<BlobManager>,
    chunks: Vec<AssetChunkDataInternal>,
}

pub struct AssetChunksReader {
    init_offset: u64,
    init_remaining: AtomicU64,
    chunk_index: AtomicUsize,
    chunks: Arc<RwLock<AssetChunks>>,
    srx: flume::Receiver<()>,
}

pub struct AssetChunks {
    key: AssetChunksCacheKey,
    chunk_bytes: u64,
    all_bytes: u64,
    source: AssetChunksSource,
    stx: flume::Sender<()>,
    srx: flume::Receiver<()>,
    manager: Weak<AssetChunksManagerInternal>,
}

struct AssetChunksManagerInternal {
    cache: RwLock<LruCache<AssetChunksCacheKey, Arc<RwLock<AssetChunks>>>>,
    blob_manager: RwLock<Option<Arc<BlobManager>>>,
}

pub struct AssetChunksManager {
    internal: Arc<AssetChunksManagerInternal>,
}

impl AssetChunksSource {
    pub fn new(blob: Arc<BlobManager>) -> Self {
        Self {
            blob,
            chunks: Default::default(),
        }
    }

    fn chunk_len(&self) -> usize {
        self.chunks.len()
    }

    fn read_chunk(&self, index: usize) -> BResult<Option<AssetChunkData>> {
        if index >= self.chunks.len() {
            return Ok(None);
        }

        let data = self.chunks[index].clone();
        match data {
            AssetChunkDataInternal::Status(asset_load_status) => {
                Ok(Some(AssetChunkData::Status(asset_load_status)))
            }
            AssetChunkDataInternal::Buffer(id) => {
                let data = self.blob.read(id)?;
                Ok(Some(AssetChunkData::Buffer(data)))
            }
        }
    }

    fn add_chunk(&mut self, chunk: AssetChunkData) -> BResult<usize> {
        let index = self.chunks.len();
        match chunk {
            AssetChunkData::Status(asset_load_status) => {
                self.chunks
                    .push(AssetChunkDataInternal::Status(asset_load_status));
            }
            AssetChunkData::Buffer(data) => {
                let id = self.blob.write(data)?;
                self.chunks.push(AssetChunkDataInternal::Buffer(id));
            }
        }
        Ok(index)
    }

    fn remove(&mut self) -> BResult<()> {
        for chunk in self.chunks.iter() {
            match chunk {
                AssetChunkDataInternal::Status(_) => {}
                AssetChunkDataInternal::Buffer(id) => {
                    self.blob.remove(*id)?;
                }
            }
        }
        self.chunks.clear();
        Ok(())
    }
}

impl AssetChunksReader {
    pub fn new(chunks: &Arc<RwLock<AssetChunks>>, byte_offset: u64) -> Arc<AssetChunksReader> {
        let reader = AssetChunksReader {
            init_offset: byte_offset,
            init_remaining: AtomicU64::new(byte_offset),
            chunk_index: AtomicUsize::new(0),
            chunks: chunks.clone(),
            srx: chunks.read().unwrap().srx.clone(),
        };
        Arc::new(reader)
    }

    pub fn all_bytes(&self) -> u64 {
        self.chunks.read().unwrap().all_bytes() - self.init_offset
    }

    pub async fn read(&self) -> BResult<Option<Vec<u8>>> {
        loop {
            let index = self.chunk_index.load(std::sync::atomic::Ordering::Relaxed);
            let chunk = {
                let chunks = self.chunks.read().unwrap();

                let chunk = chunks.source.read_chunk(index)?;
                let can_read = index < chunks.source.chunk_len();

                if can_read {
                    assert!(chunk.is_some());
                    chunk
                } else {
                    None
                }
            };

            if let Some(chunk) = chunk {
                self.chunk_index
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                match chunk {
                    AssetChunkData::Status(asset_load_status) => {
                        return match asset_load_status {
                            AssetLoadStatus::Loaded => Ok(None),
                            AssetLoadStatus::NotFound => Err(BError::AssetNotFound),
                            AssetLoadStatus::Error(e) => Err(BError::AssetLoadFail(e)),
                        }
                    }
                    AssetChunkData::Buffer(chunk) => {
                        let len = chunk.len() as u64;
                        let offset = len.min(
                            self.init_remaining
                                .load(std::sync::atomic::Ordering::Relaxed),
                        );
                        self.init_remaining
                            .fetch_sub(offset, std::sync::atomic::Ordering::Relaxed);
                        if offset == 0 {
                            return Ok(Some(chunk));
                        } else if offset < len {
                            return Ok(Some(chunk[(offset as usize)..].to_vec()));
                        }
                    }
                }
            } else {
                let any_wait = !self.srx.is_empty();
                self.srx.drain();
                if !any_wait {
                    self.srx.recv_async().await.unwrap();
                }
            }
        }
    }
}

impl AssetChunks {
    fn new(
        key: AssetChunksCacheKey,
        source: AssetChunksSource,
        manager: Weak<AssetChunksManagerInternal>,
    ) -> Arc<RwLock<Self>> {
        let (stx, srx) = flume::unbounded();
        Arc::new(RwLock::new(Self {
            key,
            chunk_bytes: 0,
            all_bytes: 0,
            source,
            stx,
            srx,
            manager,
        }))
    }

    pub fn push(&mut self, chunk: AssetChunkData) -> BResult<()> {
        let byte = match &chunk {
            AssetChunkData::Buffer(buf) => buf.len() as u64,
            _ => 0,
        };

        if let AssetChunkData::Status(status) = &chunk {
            match status {
                AssetLoadStatus::NotFound | AssetLoadStatus::Error(_) => {
                    if let Some(manager) = self.manager.upgrade() {
                        manager.evict(&self.key);
                    }
                }
                AssetLoadStatus::Loaded => {}
            }
        }

        self.source.add_chunk(chunk)?;
        self.chunk_bytes += byte;
        self.stx.send(()).unwrap();
        Ok(())
    }

    pub fn current_bytes(&self) -> u64 {
        self.chunk_bytes
    }

    pub fn all_bytes(&self) -> u64 {
        self.all_bytes
    }

    pub fn set_all_bytes(&mut self, val: u64) {
        self.all_bytes = val;
    }
}

impl Drop for AssetChunks {
    fn drop(&mut self) {
        self.source.remove().unwrap();
    }
}

impl AssetChunksManagerInternal {
    fn new(capacity: usize) -> Self {
        Self {
            cache: RwLock::new(LruCache::new(NonZero::new(capacity).unwrap())),
            blob_manager: Default::default(),
        }
    }

    fn init(&self, dir: String) {
        let mut w = self.blob_manager.write().unwrap();
        *w = Some(BlobManager::open(dir));
    }

    fn evict(&self, key: &AssetChunksCacheKey) {
        let mut w = self.cache.write().unwrap();
        w.pop(&key);
    }
}

impl AssetChunksManager {
    pub fn new(capacity: usize) -> Self {
        Self {
            internal: Arc::new(AssetChunksManagerInternal::new(capacity)),
        }
    }

    pub fn init(&self, dir: String) {
        self.internal.init(dir);
    }

    pub fn evict(&self, id: MusicId, byte_offset: u64) {
        self.internal
            .evict(&AssetChunksCacheKey { id, byte_offset });
    }

    pub fn get_or_create(&self, id: MusicId, byte_offset: u64) -> (Arc<RwLock<AssetChunks>>, bool) {
        let key = AssetChunksCacheKey { id, byte_offset };
        let (chunks, existed) = {
            let mut w = self.internal.cache.write().unwrap();
            if let Some(existed) = w.get(&key) {
                (existed.clone(), true)
            } else {
                let blob_manager = self.internal.blob_manager.write().unwrap().clone().unwrap();
                let source = AssetChunksSource::new(blob_manager);
                let created = w
                    .get_or_insert(key.clone(), || {
                        AssetChunks::new(key.clone(), source, Arc::downgrade(&self.internal))
                    })
                    .clone();
                (created, false)
            }
        };
        (chunks, existed)
    }
}
