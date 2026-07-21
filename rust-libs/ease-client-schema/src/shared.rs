use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($s:ident) => {
        #[derive(
            Debug,
            Clone,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Copy,
            uniffi::Record,
            Serialize,
            Deserialize,
        )]
        pub struct $s {
            value: i64,
        }

        impl $s {
            pub fn wrap(value: i64) -> Self {
                Self { value }
            }
        }

        impl AsRef<i64> for $s {
            fn as_ref(&self) -> &i64 {
                &self.value
            }
        }
    };
}

define_id!(StorageId);
define_id!(BlobId);
define_id!(MusicId);
define_id!(PlaylistId);

impl StorageId {
    /// Sentinel id for the synthetic, always-present Local storage.
    ///
    /// Negative so it can never collide with a real auto-incremented row in
    /// the `storage` table. The Local storage is not persisted — it is
    /// injected by the biz layer on every `list_storage` call, so callers can
    /// always resolve it regardless of DB / migration state.
    pub fn local() -> Self {
        Self::wrap(-1)
    }

    /// True if this id refers to the synthetic Local storage.
    pub fn is_local(self) -> bool {
        self == Self::local()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_storage_id_is_negative_sentinel() {
        let local = StorageId::local();
        assert!(local.is_local());
        assert_eq!(*local.as_ref(), -1);
        // Must never collide with a real auto-increment id (always >= 1).
        assert_ne!(StorageId::wrap(0), local);
        assert_ne!(StorageId::wrap(1), local);
        assert_ne!(StorageId::wrap(i64::MAX), local);
    }
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Hash,
    PartialEq,
    Eq,
    uniffi::Record,
    PartialOrd,
    Ord,
)]
pub struct StorageEntryLoc {
    pub storage_id: StorageId,
    pub path: String,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Default,
    Hash,
    Serialize,
    Deserialize,
    uniffi::Enum,
)]
pub enum StorageType {
    Local,
    #[default]
    Webdav,
    OneDrive,
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    uniffi::Enum,
)]
pub enum PlayMode {
    #[default]
    Single,
    SingleLoop,
    List,
    ListLoop,
}
