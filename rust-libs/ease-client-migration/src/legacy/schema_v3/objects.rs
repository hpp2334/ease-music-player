pub use crate::legacy::schema_v2::{
    BlobId, MusicId, PlayMode, PlaylistId, StorageEntryLoc, StorageId, StorageType,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DataSourceKey {
    Music { id: MusicId },
    Cover { id: MusicId },
    AnyEntry { entry: StorageEntryLoc },
}
