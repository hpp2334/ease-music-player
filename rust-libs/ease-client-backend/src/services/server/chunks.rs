use std::sync::{
    atomic::{AtomicU64, AtomicUsize},
    Arc, RwLock,
};

use ease_client_shared::backends::storage::{BlobId, DataSourceKey};
use serde::{Deserialize, Serialize};

use crate::{
    error::{BError, BResult},
    repositories::blob::BlobManager,
};

#[derive(Debug, bitcode::Encode, bitcode::Decode, PartialEq, Eq, uniffi::Enum)]
pub enum AssetLoadStatus {
    Loaded,
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct AssetChunksCacheKey {
    pub key: DataSourceKey,
    pub byte_offset: u64,
}

#[derive(Debug, bitcode::Encode, bitcode::Decode, uniffi::Enum)]
pub enum AssetChunkData {
    Status(AssetLoadStatus),
    Buffer(Vec<u8>),
}

pub struct AssetChunksSource {
    blob: Arc<BlobManager>,
    ids: Vec<BlobId>,
}

pub struct AssetChunksReader {
    init_offset: u64,
    init_remaining: AtomicU64,
    chunk_index: AtomicUsize,
    chunks: Arc<RwLock<AssetChunks>>,
    srx: flume::Receiver<()>,
}

pub struct AssetChunks {
    chunk_bytes: u64,
    all_bytes: u64,
    source: AssetChunksSource,
    stx: flume::Sender<()>,
    srx: flume::Receiver<()>,
}

impl AssetChunksSource {
    pub fn new(blob: Arc<BlobManager>) -> Self {
        Self {
            blob,
            ids: Default::default(),
        }
    }

    fn chunk_len(&self) -> usize {
        self.ids.len()
    }

    fn read_chunk(&self, index: usize) -> BResult<Option<AssetChunkData>> {
        if index >= self.ids.len() {
            return Ok(None);
        }

        let id = self.ids[index];
        let data = self.blob.read(id)?;
        let data = bitcode::decode(&data).unwrap();
        Ok(Some(data))
    }

    fn add_chunk(&mut self, chunk: AssetChunkData) -> BResult<usize> {
        let data = bitcode::encode(&chunk);
        let index = self.ids.len();
        let id = self.blob.write(data)?;
        self.ids.push(id);
        Ok(index)
    }

    fn remove(&mut self) -> BResult<()> {
        for id in self.ids.iter() {
            self.blob.remove(*id)?;
        }
        self.ids.clear();
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
    pub fn new(source: AssetChunksSource) -> Arc<RwLock<Self>> {
        let (stx, srx) = flume::unbounded();
        Arc::new(RwLock::new(Self {
            chunk_bytes: 0,
            all_bytes: 0,
            source,
            stx,
            srx,
        }))
    }

    pub fn push(&mut self, chunk: AssetChunkData) -> BResult<()> {
        let byte = match &chunk {
            AssetChunkData::Buffer(buf) => buf.len() as u64,
            _ => 0,
        };

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
