use std::io::SeekFrom;

use bytes::Bytes;
use ease_client_tokio::tokio_runtime;
use futures_util::future::BoxFuture;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{Entry, StorageBackend, StorageBackendError, StorageBackendResult, StreamFile};

pub struct LocalBackend;

static ANDROID_PREFIX_PATH: &str = "/storage/emulated/0";

/// Chunk size pushed per channel message when streaming a local file.
const STREAM_CHUNK_SIZE: usize = 256 * 1024;

/// Outstanding chunks allowed in flight before the reader task parks
/// (backpressure — playback consumes far slower than a local disk reads,
/// and `bytes()`-style collectors simply concatenate).
const STREAM_CHANNEL_CAP: usize = 4;

impl Default for LocalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalBackend {
    pub fn new() -> Self {
        Self
    }

    async fn list_impl(&self, dir: String) -> StorageBackendResult<Vec<Entry>> {
        let dir = if std::env::consts::OS == "windows" {
            dir.replace('/', "\\")
        } else if std::env::consts::OS == "android" {
            ANDROID_PREFIX_PATH.to_string() + dir.as_str()
        } else {
            dir.to_string()
        };

        let mut ret = tokio_runtime()
            .spawn(async move {
                let path = tokio::fs::canonicalize(dir).await?;
                let mut dir = tokio::fs::read_dir(path).await?;

                let mut ret: Vec<Entry> = Default::default();
                while let Some(entry) = dir.next_entry().await? {
                    let metadata = entry.metadata().await?;
                    let mut path = entry
                        .path()
                        .to_string_lossy()
                        .to_string()
                        .replace("\\\\?\\", "");
                    if std::env::consts::OS == "android" {
                        if let Some(strip_path) = path.strip_prefix(ANDROID_PREFIX_PATH) {
                            path = strip_path.to_string();
                        }
                    }

                    ret.push(Entry {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: path.replace('\\', "/"),
                        size: Some(metadata.len() as usize),
                        is_dir: metadata.is_dir(),
                    });
                }

                Ok::<_, StorageBackendError>(ret)
            })
            .await??;

        ret.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ret)
    }

    /// Stream a local file in bounded chunks instead of buffering it whole
    /// into RAM. The open phase (canonicalize + open + stat + seek) runs to
    /// completion so missing-file / access errors surface from `get` itself;
    /// then a spawned reader task pushes fixed-size chunks through a bounded
    /// channel — the same delivery model the JS storage-provider plugins use
    /// ([`StreamFile::new_from_rx`]). Mid-stream io errors travel over the
    /// channel as `Err` so consumers (playback, `bytes()`) see them instead
    /// of a silent truncation.
    async fn get_impl(&self, p: String, byte_offset: u64) -> StorageBackendResult<StreamFile> {
        let p = if std::env::consts::OS == "windows" {
            p.replace('/', "\\")
        } else if std::env::consts::OS == "android" {
            ANDROID_PREFIX_PATH.to_string() + p.as_str()
        } else {
            p.to_string()
        };

        let (file, total) = {
            let p = p.clone();
            tokio_runtime()
                .spawn(async move {
                    let path = tokio::fs::canonicalize(&p).await?;
                    let mut file = tokio::fs::File::open(&path).await?;
                    let total = file.metadata().await?.len() as usize;
                    file.seek(SeekFrom::Start(byte_offset)).await?;
                    Ok::<_, StorageBackendError>((file, total))
                })
                .await??
        };

        let (tx, rx) = async_channel::bounded::<StorageBackendResult<Bytes>>(STREAM_CHANNEL_CAP);

        tokio_runtime().spawn(async move {
            let mut file = file;
            let mut buf = vec![0u8; STREAM_CHUNK_SIZE];
            loop {
                match file.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(Ok(Bytes::copy_from_slice(&buf[..n]))).await.is_err() {
                            // Receiver dropped (consumer cancelled) — stop reading.
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(StorageBackendError::TokioIO(e))).await;
                        break;
                    }
                }
            }
            let _ = tx.close();
        });

        // `total` is the FULL file length (for `total_size`); the chunks
        // already start at `byte_offset` — exactly the `new_from_rx`
        // contract (shared with the plugin byte bridge).
        Ok(StreamFile::new_from_rx(rx, Some(total), byte_offset, &p, None))
    }
}

impl StorageBackend for LocalBackend {
    fn list(&self, dir: String) -> BoxFuture<StorageBackendResult<Vec<Entry>>> {
        Box::pin(self.list_impl(dir))
    }
    fn get(&self, p: String, byte_offset: u64) -> BoxFuture<StorageBackendResult<StreamFile>> {
        Box::pin(self.get_impl(p, byte_offset))
    }
}

#[cfg(test)]
mod test {
    use super::STREAM_CHUNK_SIZE;
    use crate::{LocalBackend, StorageBackend};

    #[tokio::test]
    async fn test_list_dir() {
        let backend = LocalBackend::new();

        let cwd = std::env::current_dir()
            .unwrap()
            .join("test/assets/case_list");
        let cwd = cwd.to_string_lossy().to_string();
        let list = backend.list(cwd).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "a.txt");
        assert_eq!(list[1].name, "b.log.txt");
    }

    #[tokio::test]
    async fn test_list_dir_use_linux_slash() {
        let backend = LocalBackend::new();

        let cwd = std::env::current_dir()
            .unwrap()
            .join("test/assets/case_list");
        let cwd = cwd.to_string_lossy().to_string();
        let cwd = cwd.replace("\\", "/");
        let list = backend.list(cwd).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "a.txt");
        assert_eq!(list[1].name, "b.log.txt");
    }

    #[tokio::test]
    async fn test_partial_bytes() {
        let backend = LocalBackend::new();

        let cwd = std::env::current_dir()
            .unwrap()
            .join("test/assets/case_list/b.log.txt");
        let cwd = cwd.to_string_lossy().to_string();
        let file = backend.get(cwd, 3).await.unwrap();
        let bytes = file.bytes().await.unwrap();

        assert_eq!(String::from_utf8_lossy(bytes.as_ref()), "og.txt");
    }

    #[tokio::test]
    async fn test_partial_stream() {
        let backend = LocalBackend::new();

        let cwd = std::env::current_dir()
            .unwrap()
            .join("test/assets/case_list/b.log.txt");
        let cwd = cwd.to_string_lossy().to_string();
        let file = backend.get(cwd, 3).await.unwrap();

        let stream = file.into_rx();
        let chunk = stream.recv().await;
        assert!(chunk.is_ok());
        let chunk = chunk.unwrap().unwrap();
        assert_eq!(String::from_utf8_lossy(chunk.as_ref()), "og.txt");
    }

    #[tokio::test]
    async fn test_stream_reports_full_and_remaining_size() {
        let backend = LocalBackend::new();

        let path = std::env::current_dir()
            .unwrap()
            .join("test/assets/case_list/b.log.txt");
        let full_len = std::fs::read(&path).unwrap().len();
        let file = backend
            .get(path.to_string_lossy().to_string(), 3)
            .await
            .unwrap();

        // total_size ignores the offset; size reports the remaining bytes.
        assert_eq!(file.total_size(), Some(full_len));
        assert_eq!(file.size(), Some(full_len - 3));
    }

    #[tokio::test]
    async fn test_stream_multi_chunk_roundtrip() {
        let backend = LocalBackend::new();

        // A file larger than one stream chunk exercises the multi-push +
        // concatenation path with backpressure in play.
        let content_len = STREAM_CHUNK_SIZE + STREAM_CHUNK_SIZE / 2;
        let content: Vec<u8> = (0..content_len).map(|i| (i % 251) as u8).collect();
        let path = std::env::temp_dir().join(format!(
            "ease_local_stream_test_{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, &content).unwrap();

        let file = backend
            .get(path.to_string_lossy().to_string(), 0)
            .await
            .unwrap();
        assert_eq!(file.total_size(), Some(content_len));

        // Collect via the channel and count the chunks (>= 2 expected).
        let rx = file.into_rx();
        let mut collected: Vec<u8> = Vec::with_capacity(content_len);
        let mut chunks = 0usize;
        while let Ok(chunk) = rx.recv().await {
            let chunk = chunk.unwrap();
            chunks += 1;
            collected.extend_from_slice(&chunk);
        }
        assert!(chunks >= 2, "expected multi-chunk delivery, got {chunks}");
        assert_eq!(collected, content);

        // And via `bytes()` (the collect-all consumer).
        let file = backend
            .get(path.to_string_lossy().to_string(), 4096)
            .await
            .unwrap();
        let bytes = file.bytes().await.unwrap();
        assert_eq!(bytes.as_ref(), &content[4096..]);

        let _ = std::fs::remove_file(&path);
    }
}
