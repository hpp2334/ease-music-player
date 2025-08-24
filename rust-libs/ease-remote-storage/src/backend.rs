use std::{io::ErrorKind, process::Output};

use bytes::Bytes;
use ease_client_tokio::tokio_runtime;
use futures_util::future::BoxFuture;
use reqwest::StatusCode;
use tokio::sync::oneshot::error;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub size: Option<usize>,
    pub is_dir: bool,
}

enum StreamFileInner {
    Response(reqwest::Response),
    Total(bytes::Bytes),
}

pub struct StreamFile {
    inner: StreamFileInner,
    total: Option<usize>,
    content_type: Option<String>,
    name: String,
    byte_offset: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum StorageBackendError {
    #[error(transparent)]
    RequestFail(#[from] reqwest::Error),
    #[error("Parse XML Fail")]
    ParseXMLFail,
    #[error(transparent)]
    TokioIO(#[from] tokio::io::Error),
    #[error(transparent)]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("Url Parse Error")]
    UrlParseError(String),
    #[error("Serde Json Error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(thiserror::Error, Debug)]
enum SendChunkError {
    #[error(transparent)]
    RequestFail(#[from] reqwest::Error),
    #[error("mpsc send error: {0}")]
    MpscSendError(#[from] async_channel::SendError<StorageBackendResult<Bytes>>),
}

pub type StorageBackendResult<T> = std::result::Result<T, StorageBackendError>;

impl StorageBackendError {
    pub fn is_timeout(&self) -> bool {
        if let StorageBackendError::RequestFail(e) = self {
            return e.is_timeout();
        }
        false
    }

    pub fn is_unauthorized(&self) -> bool {
        if let StorageBackendError::RequestFail(e) = self {
            return e.status() == Some(StatusCode::UNAUTHORIZED);
        }
        false
    }

    pub fn is_not_found(&self) -> bool {
        match self {
            StorageBackendError::RequestFail(e) => e.status() == Some(StatusCode::NOT_FOUND),
            StorageBackendError::TokioIO(e) => e.kind() == ErrorKind::NotFound,
            _ => false,
        }
    }
}

pub trait StorageBackend {
    fn list(&self, dir: String) -> BoxFuture<StorageBackendResult<Vec<Entry>>>;
    fn get(&self, p: String, byte_offset: u64) -> BoxFuture<StorageBackendResult<StreamFile>>;
}

impl StreamFile {
    pub fn new(resp: reqwest::Response, byte_offset: u64) -> Self {
        let url = resp.url().to_string();
        let name = url.split('/').next_back().unwrap();
        let header_map = resp.headers();
        let content_length = header_map.get(reqwest::header::CONTENT_LENGTH).map(|v| {
            let v = v.to_str().unwrap();
            v.parse::<usize>().unwrap()
        });
        let content_type = header_map
            .get(reqwest::header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_string());
        Self {
            inner: StreamFileInner::Response(resp),
            total: content_length,
            content_type,
            name: name.to_string(),
            byte_offset,
        }
    }
    pub fn new_from_bytes(buf: &[u8], name: &str, byte_offset: u64) -> Self {
        let total: usize = buf.len();
        let buf = bytes::Bytes::copy_from_slice(buf);
        Self {
            inner: StreamFileInner::Total(buf),
            total: Some(total),
            content_type: None,
            name: name.to_string(),
            byte_offset: byte_offset.min(total as u64),
        }
    }
    pub fn size(&self) -> Option<usize> {
        self.total.map(|total| total - self.byte_offset as usize)
    }
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn into_rx(self) -> async_channel::Receiver<StorageBackendResult<Bytes>> {
        let (mut tx, rx) = async_channel::bounded::<StorageBackendResult<Bytes>>(10);

        let _ = tokio_runtime().spawn(async move {
            let f = || async {
                match self.inner {
                    StreamFileInner::Response(mut response) => {
                        let mut remaining = self.byte_offset as usize;

                        while let Some(chunk) = response.chunk().await? {
                            if chunk.len() <= remaining {
                                remaining -= chunk.len();
                            } else if remaining > 0 {
                                let chunk = Bytes::copy_from_slice(&chunk[remaining..]);
                                remaining = 0;
                                tx.send(Ok(chunk)).await?;
                            } else {
                                tx.send(Ok(chunk)).await?;
                            }
                        }
                    }
                    StreamFileInner::Total(buf) => {
                        let offset = self.byte_offset as usize;
                        if offset == 0 {
                            tx.send(Ok(buf)).await?;
                        } else {
                            let buf = Bytes::copy_from_slice(&buf[offset..]);
                            tx.send(Ok(buf)).await?;
                        }
                    }
                }

                Ok(())
            };

            let res: Result<(), SendChunkError> = f().await;
            if let Err(e) = res {
                let e: Option<StorageBackendError> = match e {
                    SendChunkError::RequestFail(e) => Some(e.into()),
                    _ => None,
                };
                if let Some(e) = e {
                    let _ = tx.send(Err(e)).await;
                }
            }
            let _ = tx.close();
        });

        rx
    }

    pub async fn bytes(self) -> StorageBackendResult<Bytes> {
        let buf = match self.inner {
            StreamFileInner::Response(response) => response.bytes().await?,
            StreamFileInner::Total(buf) => buf,
        };

        let offset = (self.byte_offset as usize).min(buf.len());
        if offset == 0 {
            Ok(buf)
        } else {
            let buf = Bytes::copy_from_slice(&buf[offset..]);
            Ok(buf)
        }
    }
}
