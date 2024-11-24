use std::io::ErrorKind;

use async_stream::stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use reqwest::StatusCode;

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
}

#[derive(thiserror::Error, Debug)]
pub enum StorageBackendError {
    #[error(transparent)]
    RequestFail(#[from] reqwest::Error),
    #[error("Parse XML Fail")]
    ParseXMLFail,
    #[error(transparent)]
    TokioIO(#[from] tokio::io::Error),
    #[error("Url Parse Error")]
    UrlParseError(String),
    #[error("Serde Json Error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}

pub type StorageBackendResult<T> = std::result::Result<T, StorageBackendError>;

impl StorageBackendError {
    pub fn is_timeout(&self) -> bool {
        if let StorageBackendError::RequestFail(e) = self {
            return e.is_timeout();
        }
        return false;
    }

    pub fn is_unauthorized(&self) -> bool {
        if let StorageBackendError::RequestFail(e) = self {
            return e.status() == Some(StatusCode::UNAUTHORIZED);
        }
        return false;
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
    fn get(&self, p: String) -> BoxFuture<StorageBackendResult<StreamFile>>;
}

impl StreamFile {
    pub fn new(resp: reqwest::Response) -> Self {
        let url = resp.url().to_string();
        let name = url.split('/').last().unwrap();
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
        }
    }
    pub fn new_from_bytes(buf: &[u8], name: &str) -> Self {
        let total: usize = buf.len() as usize;
        let buf = bytes::Bytes::copy_from_slice(buf);
        Self {
            inner: StreamFileInner::Total(buf),
            total: Some(total),
            content_type: None,
            name: name.to_string(),
        }
    }
    pub fn size(&self) -> Option<usize> {
        self.total
    }
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_ref().map(|v| v.as_str())
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn into_stream(self) -> impl futures_util::Stream<Item = StorageBackendResult<Bytes>> {
        stream! {
            match self.inner {
                StreamFileInner::Response(mut response) => {
                        while let Some(chunk) = response.chunk().await? {
                            yield(Ok(chunk))
                        }
                    },
                StreamFileInner::Total(buf) => {
                        yield(Ok(buf));
                },
            }
        }
    }

    pub async fn bytes(self) -> StorageBackendResult<Bytes> {
        match self.inner {
            StreamFileInner::Response(response) => Ok(response.bytes().await?),
            StreamFileInner::Total(buf) => Ok(buf),
        }
    }
}
