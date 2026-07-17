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
