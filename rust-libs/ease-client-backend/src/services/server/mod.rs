mod serve;

use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, RwLock},
};

use ease_client_shared::backends::storage::DataSourceKey;
use ease_remote_storage::StreamFile;
use futures::StreamExt;
use misty_async::Task;
use serve::{
    get_stream_file_by_loc, get_stream_file_by_music_id, get_stream_file_cover_by_music_id,
};

use crate::{ctx::BackendContext, error::BResult};

#[derive(Default, uniffi::Enum)]
pub enum AssetLoadStatus {
    #[default]
    Pending,
    Loaded,
    NotFound,
    Error(String),
}

struct OpenedAsset {
    _task: Task<()>,
}

pub struct AssetServer {
    alloc: AtomicU64,
    map: RwLock<HashMap<u64, OpenedAsset>>,
    cx: Arc<BackendContext>,
}

pub trait IAssetLoadDelegate: Send + Sync + 'static {
    fn on_status(&self, status: AssetLoadStatus);
    fn on_chunk(&self, chunk: Vec<u8>);
}

impl AssetServer {
    pub fn new(cx: Arc<BackendContext>) -> Arc<Self> {
        Arc::new(Self {
            alloc: Default::default(),
            map: Default::default(),
            cx,
        })
    }

    pub async fn load(
        self: &Arc<Self>,
        key: DataSourceKey,
        byte_offset: u64,
    ) -> BResult<Option<StreamFile>> {
        match key {
            DataSourceKey::Music { id } => {
                get_stream_file_by_music_id(&self.cx, id, byte_offset).await
            }
            DataSourceKey::Cover { id } => {
                get_stream_file_cover_by_music_id(&self.cx, id, byte_offset).await
            }
            DataSourceKey::AnyEntry { entry } => {
                get_stream_file_by_loc(&self.cx, entry, byte_offset).await
            }
        }
    }

    pub fn open(
        self: &Arc<Self>,
        key: DataSourceKey,
        offset: usize,
        listener: Arc<dyn IAssetLoadDelegate>,
    ) -> u64 {
        let cx = self.cx.clone();
        let id = self
            .alloc
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let this = self.clone();
        let task = {
            cx.async_runtime().clone().spawn(async move {
                let file = this.load(key, offset as u64).await;

                if let Err(e) = &file {
                    listener.on_status(AssetLoadStatus::Error(e.to_string()));
                    return;
                }
                let file = file.unwrap();

                if file.is_none() {
                    listener.on_status(AssetLoadStatus::NotFound);
                    return;
                }

                let file = file.unwrap();
                let mut stream = Box::pin(file.into_stream());
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            listener.on_chunk(bytes.to_vec());
                        }
                        Err(err) => {
                            listener.on_status(AssetLoadStatus::Error(err.to_string()));
                            return;
                        }
                    }
                }

                listener.on_status(AssetLoadStatus::Loaded);
            })
        };
        {
            let mut w = self.map.write().unwrap();
            w.insert(id, OpenedAsset { _task: task });
        }

        id
    }

    pub fn close(self: &Arc<Self>, id: u64) {
        let mut w = self.map.write().unwrap();
        w.remove(&id);
    }
}
