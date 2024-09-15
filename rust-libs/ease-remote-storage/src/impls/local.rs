use std::sync::Mutex;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use tokio::io::AsyncReadExt;

use crate::{StorageBackend, BackendResult, Entry, StreamFile};

pub struct LocalBackend;

static ANDROID_PREFIX_PATH: &str = "/storage/emulated/0";

impl LocalBackend {
    pub fn new() -> Self {
        Self
    }

    async fn list_impl(&self, dir: &str) -> BackendResult<Vec<Entry>> {
        let dir = if std::env::consts::OS == "windows" {
            dir.replace('/', "\\")
        } else if std::env::consts::OS == "android" {
            ANDROID_PREFIX_PATH.to_string() + dir
        } else {
            dir.to_string()
        };

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
        ret.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ret)
    }

    async fn get_impl(&self, p: &str) -> BackendResult<StreamFile> {
        let p = if std::env::consts::OS == "windows" {
            p.replace('/', "\\")
        } else if std::env::consts::OS == "android" {
            ANDROID_PREFIX_PATH.to_string() + p
        } else {
            p.to_string()
        };
        let path = tokio::fs::canonicalize(&p).await?;
        let mut file = tokio::fs::File::open(path).await?;

        let mut buf: Vec<u8> = Default::default();
        file.read_to_end(&mut buf).await?;
        Ok(StreamFile::new_from_bytes(buf.as_slice(), &p))
    }
}

static LOCAL_STORAGE_PATH: Lazy<Mutex<Option<String>>> =
    Lazy::new(|| Mutex::new(Some("/".to_string())));

pub fn set_global_local_storage_path(p: String) {
    let mut guard = LOCAL_STORAGE_PATH.lock().unwrap();
    *guard = Some(p);
}

#[async_trait]
impl StorageBackend for LocalBackend {
    async fn list(&self, dir: &str) -> BackendResult<Vec<Entry>> {
        self.list_impl(dir).await
    }
    async fn remove(&self, _p: &str) {
        unimplemented!()
    }

    async fn get(&self, p: &str) -> BackendResult<StreamFile> {
        self.get_impl(p).await
    }
    fn default_url(&self) -> String {
        let p = { LOCAL_STORAGE_PATH.lock().unwrap().clone().unwrap() };
        p
    }
}

#[cfg(test)]
mod test {
    use crate::{StorageBackend, LocalBackend};

    #[tokio::test]
    async fn test_list_dir() {
        let backend = LocalBackend::new();

        let cwd = std::env::current_dir()
            .unwrap()
            .join("test/assets/case_list");
        let cwd = cwd.to_string_lossy().to_string();
        let list = backend.list(&cwd).await.unwrap();
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
        let list = backend.list(&cwd).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "a.txt");
        assert_eq!(list[1].name, "b.log.txt");
    }
}
