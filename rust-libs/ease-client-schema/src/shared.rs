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
            serde::Serialize,
            serde::Deserialize,
        )]
        #[serde(transparent)]
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

macro_rules! define_string_id {
    ($s:ident) => {
        #[derive(
            Debug,
            Clone,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $s {
            pub id: String,
        }

        impl $s {
            pub fn new(id: impl Into<String>) -> Self {
                Self { id: id.into() }
            }
        }

        impl From<String> for $s {
            fn from(id: String) -> Self {
                Self { id }
            }
        }

        impl AsRef<str> for $s {
            fn as_ref(&self) -> &str {
                &self.id
            }
        }

        impl std::fmt::Display for $s {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.id)
            }
        }
    };
}

// `StorageId` is the universal registry id (the `storage` table PK). Every
// music / playlist storage reference (`loc_storage_id`, `lyric_storage_id`,
// `picture_storage_id`) points at a `storage` row regardless of whether the
// backing source is Local, a WebDAV connection, or a plugin provider. Resolve
// the concrete backend through `obtain(StorageHandle) -> StorageId` /
// `get_storage_backend(StorageId)`.
define_id!(StorageId);
define_id!(WebdavStorageId);
define_id!(SecretId);
define_id!(BlobId);
define_id!(MusicId);
define_id!(PlaylistId);

define_string_id!(PluginId);
define_string_id!(PluginStorageId);

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[serde(rename_all = "camelCase")]
pub struct StorageEntryLoc {
    pub storage_id: StorageId,
    pub path: String,
}

/// Discriminant stored in the `storage` registry table's `type` column.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageType {
    Local,
    Webdav,
    Plugin,
}

impl StorageType {
    pub fn as_i32(self) -> i32 {
        match self {
            StorageType::Local => 0,
            StorageType::Webdav => 1,
            StorageType::Plugin => 2,
        }
    }

    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(StorageType::Local),
            1 => Some(StorageType::Webdav),
            2 => Some(StorageType::Plugin),
            _ => None,
        }
    }
}

/// Parametric descriptor of a storage source — the input to
/// `obtain(StorageHandle) -> StorageId`. The registry row is find-or-created
/// from this; the returned `StorageId` is what music / playlists persist.
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageHandle {
    Local,
    #[serde(rename_all = "camelCase")]
    Webdav {
        webdav_storage_id: WebdavStorageId,
    },
    #[serde(rename_all = "camelCase")]
    Plugin {
        plugin_id: PluginId,
        plugin_storage_id: PluginStorageId,
    },
}

impl StorageHandle {
    pub fn storage_type(&self) -> StorageType {
        match self {
            StorageHandle::Local => StorageType::Local,
            StorageHandle::Webdav { .. } => StorageType::Webdav,
            StorageHandle::Plugin { .. } => StorageType::Plugin,
        }
    }
}

/// Ownership scope of a `secret` row. Persisted in the `secret.scope` TEXT
/// column as `"internal"` (host-owned, e.g. a WebDAV password) or
/// `"plugin:<plugin_id>"` (owned by that plugin). Enforcement: a caller may
/// only `get` / `remove` a secret whose scope matches its own.
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum SecretScope {
    Internal,
    Plugin(PluginId),
}

impl SecretScope {
    /// The string stored in the `secret.scope` column.
    pub fn to_scope_string(&self) -> String {
        match self {
            SecretScope::Internal => "internal".to_string(),
            SecretScope::Plugin(pid) => format!("plugin:{}", pid.id),
        }
    }

    /// Parse a `secret.scope` column value back into a `SecretScope`.
    /// Unknown values default to `Internal` (defensive: a corrupt row becomes
    /// host-owned rather than accessible to an arbitrary plugin).
    pub fn from_scope_string(s: &str) -> Self {
        if let Some(rest) = s.strip_prefix("plugin:") {
            SecretScope::Plugin(PluginId::new(rest))
        } else {
            SecretScope::Internal
        }
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayMode {
    #[default]
    Single,
    SingleLoop,
    List,
    ListLoop,
}
