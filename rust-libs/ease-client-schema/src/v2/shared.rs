use std::time::Duration;

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
            bitcode::Encode,
            bitcode::Decode,
            uniffi::Record,
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

        impl serde::Serialize for $s {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.value.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $s {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                i64::deserialize(deserializer).map(|p| Self { value: p })
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
    bitcode::Encode,
    bitcode::Decode,
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
    bitcode::Decode,
    bitcode::Encode,
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
    bitcode::Decode,
    bitcode::Encode,
)]
pub enum PlayMode {
    #[default]
    Single,
    SingleLoop,
    List,
    ListLoop,
}

#[derive(Debug, Clone, Copy, bitcode::Encode, bitcode::Decode)]
pub struct MusicDuration(pub Duration);
