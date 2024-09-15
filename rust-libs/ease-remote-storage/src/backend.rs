use std::io::ErrorKind;

use async_stream::stream;
use async_trait::async_trait;
use bytes::Bytes;
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
    url: String,
}

#[derive(thiserror::Error, Debug)]
pub enum BackendError {
    #[error(transparent)]
    RequestFail(#[from] reqwest::Error),
    #[error("Parse XML Fail")]
    ParseXMLFail,
    #[error(transparent)]
    TokioIO(#[from] tokio::io::Error),
    #[error("Url Parse Error")]
    UrlParseError(String),
}

pub type BackendResult<T> = std::result::Result<T, BackendError>;

impl BackendError {
    pub fn is_timeout(&self) -> bool {
        if let BackendError::RequestFail(e) = self {
            return e.is_timeout();
        }
        return false;
    }

    pub fn is_unauthorized(&self) -> bool {
        if let BackendError::RequestFail(e) = self {
            return e.status() == Some(StatusCode::UNAUTHORIZED);
        }
        return false;
    }

    pub fn is_not_found(&self) -> bool {
        match self {
            BackendError::RequestFail(e) => e.status() == Some(StatusCode::NOT_FOUND),
            BackendError::TokioIO(e) => e.kind() == ErrorKind::NotFound,
            _ => false,
        }
    }
}

#[async_trait]
pub trait StorageBackend {
    async fn list(&self, dir: &str) -> BackendResult<Vec<Entry>>;
    async fn remove(&self, p: &str);
    async fn get(&self, p: &str) -> BackendResult<StreamFile>;
    fn default_url(&self) -> String;
}

impl StreamFile {
    pub fn new(resp: reqwest::Response) -> Self {
        let url = resp.url().to_string();
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
            url,
        }
    }
    pub fn new_from_bytes(buf: &[u8], p: &str) -> Self {
        let total: usize = buf.len() as usize;
        let buf = bytes::Bytes::copy_from_slice(buf);
        Self {
            inner: StreamFileInner::Total(buf),
            total: Some(total),
            content_type: None,
            url: p.to_string(),
        }
    }
    pub fn get_size(&self) -> Option<usize> {
        self.total
    }
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_ref().map(|v| v.as_str())
    }
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn into_stream(self) -> impl futures_util::Stream<Item = BackendResult<Bytes>> {
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

    pub async fn chunk_small(self) -> BackendResult<Bytes> {
        const N: usize = 6_000_000;
        match self.inner {
            StreamFileInner::Response(mut response) => {
                let mut ret: Vec<u8> = Default::default();
                while let Some(buf) = response.chunk().await? {
                    ret.append(&mut buf.to_vec());

                    if ret.len() >= N {
                        break;
                    }
                }
                return Ok(Bytes::from(ret));
            }
            StreamFileInner::Total(buf) => {
                return if buf.len() < N {
                    Ok(buf)
                } else {
                    Ok(Bytes::copy_from_slice(&buf[0..N]))
                }
            }
        }
    }

    pub async fn bytes(self) -> BackendResult<Bytes> {
        match self.inner {
            StreamFileInner::Response(response) => Ok(response.bytes().await?),
            StreamFileInner::Total(buf) => Ok(buf),
        }
    }
}
