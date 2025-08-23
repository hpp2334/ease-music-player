use std::io::SeekFrom;

use ease_client_tokio::tokio_runtime;
use futures_util::future::BoxFuture;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{Entry, StorageBackend, StorageBackendError, StorageBackendResult, StreamFile};

pub struct LocalBackend;

static ANDROID_PREFIX_PATH: &str = "/storage/emulated/0";

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

    async fn get_impl(&self, p: String, byte_offset: u64) -> StorageBackendResult<StreamFile> {
        let p = if std::env::consts::OS == "windows" {
            p.replace('/', "\\")
        } else if std::env::consts::OS == "android" {
            ANDROID_PREFIX_PATH.to_string() + p.as_str()
        } else {
            p.to_string()
        };

        let buf = {
            let p = p.clone();
            tokio_runtime()
                .spawn(async move {
                    let mut buf: Vec<u8> = Default::default();
                    let path = tokio::fs::canonicalize(&p).await?;
                    let mut file = tokio::fs::File::open(path).await?;

                    file.seek(SeekFrom::Start(byte_offset)).await?;
                    file.read_to_end(&mut buf).await?;

                    Ok::<_, StorageBackendError>(buf)
                })
                .await??
        };

        Ok(StreamFile::new_from_bytes(buf.as_slice(), &p, 0))
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
    use futures_util::{pin_mut, StreamExt};

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

        let stream = file.into_stream();
        pin_mut!(stream);
        let chunk = stream.next().await;
        assert!(chunk.is_some());
        let chunk = chunk.unwrap().unwrap();
        assert_eq!(String::from_utf8_lossy(chunk.as_ref()), "og.txt");
    }
}
