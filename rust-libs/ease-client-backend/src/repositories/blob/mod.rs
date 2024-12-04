use std::{path::PathBuf, sync::Arc};

use ease_client_shared::backends::storage::BlobId;
use redb::{ReadableTable, TableDefinition};

use crate::error::BResult;

use super::bin::BinSerde;

pub struct BlobManager {
    dir: String,
    db: redb::Database,
}

const TABLE_BLOB: TableDefinition<(), BinSerde<BlobId>> = TableDefinition::new("blob");

fn blobs_path(dir: &str) -> PathBuf {
    std::path::Path::new(dir).join("blobs")
}

fn db_path(dir: &str) -> PathBuf {
    std::path::Path::new(dir).join("blob.redb")
}

fn blob_path(dir: &str, index: BlobId) -> PathBuf {
    std::path::Path::new(dir)
        .join("blobs")
        .join(index.as_ref().to_string())
}

impl BlobManager {
    pub fn open(dir: String) -> Arc<Self> {
        std::fs::create_dir_all(&dir).expect("Failed to create directory");
        std::fs::create_dir_all(blobs_path(&dir)).expect("Failed to create directory");
        let db = redb::Database::builder()
            .set_cache_size(20 << 20)
            .create(db_path(&dir))
            .unwrap();
        let ret = Arc::new(Self { dir, db });
        ret
    }

    pub fn read(&self, id: BlobId) -> BResult<Vec<u8>> {
        let path = blob_path(self.dir.as_str(), id);
        let ret = std::fs::read(path)?;
        Ok(ret)
    }

    pub fn remove(&self, id: BlobId) -> BResult<()> {
        let path = blob_path(self.dir.as_str(), id);
        std::fs::remove_file(path)?;
        Ok(())
    }

    pub fn write(&self, buf: Vec<u8>) -> BResult<BlobId> {
        let id = self.allocate()?;
        let path = blob_path(self.dir.as_str(), id);
        std::fs::write(path, buf)?;
        Ok(id)
    }

    // pub fn clear(&self) -> BResult<()> {
    //     let paths = std::fs::read_dir(blobs_path(&self.dir)).expect("Failed to read directory");
    //     for path in paths {
    //         let path = path.expect("Failed to get path").path();
    //         std::fs::remove_file(path).expect("Failed to remove file");
    //     }

    //     {
    //         let txn = self.db.begin_write()?;
    //         {
    //             let mut table = txn.open_table(TABLE_BLOB)?;
    //             table.insert((), BlobId::wrap(0))?;
    //         };
    //         txn.commit()?;
    //     }
    //     Ok(())
    // }

    fn allocate(&self) -> BResult<BlobId> {
        let txn = self.db.begin_write()?;
        let id = {
            let mut table = txn.open_table(TABLE_BLOB)?;
            let id = table.get(())?.map(|v| v.value()).unwrap_or(BlobId::wrap(0));
            let next_id = BlobId::wrap(*id.as_ref() + 1);
            table.insert((), next_id)?;
            id
        };
        txn.commit()?;
        Ok(id)
    }
}
