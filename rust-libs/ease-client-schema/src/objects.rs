use crate::shared::{MusicId, StorageEntryLoc};

#[derive(Debug, Clone, Hash, PartialEq, Eq, uniffi::Enum)]
pub enum DataSourceKey {
    Music { id: MusicId },
    Cover { id: MusicId },
    AnyEntry { entry: StorageEntryLoc },
}
