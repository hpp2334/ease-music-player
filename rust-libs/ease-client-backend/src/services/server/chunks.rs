use std::sync::{atomic::AtomicU64, Arc, RwLock};

use ease_client_shared::backends::storage::DataSourceKey;
use serde::{Deserialize, Serialize};

use crate::error::{BError, BResult};

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

#[derive(Clone)]
pub struct AssetChunksSource {
    key: String,
    db: Arc<redb::Database>,
}

pub struct AssetChunksReader {
    init_offset: u64,
    init_remaining: AtomicU64,
    chunk_index: AtomicU64,
    chunks: Arc<RwLock<AssetChunks>>,
    srx: flume::Receiver<()>,
}

pub struct AssetChunks {
    chunk_count: u64,
    chunk_bytes: u64,
    all_bytes: u64,
    source: AssetChunksSource,
    stx: flume::Sender<()>,
    srx: flume::Receiver<()>,
}

impl AssetChunksSource {
    pub fn new(source_id: u64, db: Arc<redb::Database>) -> Self {
        Self {
            key: source_id.to_string(),
            db,
        }
    }

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
            let data = bitcode::decode(&data.value()).unwrap();
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
            let data = bitcode::encode(&chunk);
            let old = table.insert(index, data)?;
            assert!(old.is_none());
        }
        w_txn.commit()?;

        Ok(())
    }

    fn remove(&self) -> BResult<()> {
        let w_txn = self.db.begin_write()?;
        let table_definition = Self::def(&self.key);
        w_txn.delete_table(table_definition)?;
        w_txn.commit()?;
        Ok(())
    }
}

impl AssetChunksReader {
    pub fn new(chunks: &Arc<RwLock<AssetChunks>>, byte_offset: u64) -> Arc<AssetChunksReader> {
        let reader = AssetChunksReader {
            init_offset: byte_offset,
            init_remaining: AtomicU64::new(byte_offset),
            chunk_index: AtomicU64::new(0),
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
                let can_read = index < chunks.chunk_count;

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
            chunk_count: 0,
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

        let index = self.chunk_count;
        self.chunk_count += 1;
        self.source.add_chunk(index, chunk)?;
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
