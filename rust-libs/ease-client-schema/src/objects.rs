use crate::shared::{MusicId, StorageEntryLoc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataSourceKey {
    Music { id: MusicId },
    Cover { id: MusicId },
    AnyEntry { entry: StorageEntryLoc },
}

// ============================================================================
// Plugin KV storage
//
// Three normalized tables back a unified key/value API for plugins:
//   - plugin_kv_key    : master registry of (plugin_id, key) pairs, tagged
//                        with KvKind (Single or Multi). The auto-increment
//                        `id` is the FK the other two tables reference.
//   - plugin_kv_single : overwrite-mode values (one row per key_id).
//   - plugin_kv_multi  : append-only event log (many rows per key_id).
//
// A key is locked to its kind on first use; mixing kinds on the same
// (plugin_id, key) returns an error from the controller layer.
// ============================================================================

/// Storage mode for a plugin KV key. Locked at first use per
/// (plugin_id, key): a key declared as Single cannot later receive
/// appends (and vice versa).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PluginKvKind {
    /// One value per key. `set` overwrites.
    Single,
    /// Many values per key. `append` adds a row; reads return lists.
    Multi,
}

impl PluginKvKind {
    pub fn as_i32(self) -> i32 {
        match self {
            PluginKvKind::Single => 0,
            PluginKvKind::Multi => 1,
        }
    }

    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(PluginKvKind::Single),
            1 => Some(PluginKvKind::Multi),
            _ => None,
        }
    }
}

/// One (key, value) pair for the single-value API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginKvEntry {
    pub key: String,
    pub value: String,
}

/// One (key, values) pair returned by multi-value bulk reads. `values`
/// is ordered oldest-first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginKvMultiEntry {
    pub key: String,
    pub values: Vec<String>,
}

/// One (key, count) pair returned by `count_multi`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginKvCountEntry {
    pub key: String,
    pub count: u64,
}

/// Metadata about a registered key, returned by `list_keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginKvKeyInfo {
    pub key: String,
    pub kind: PluginKvKind,
}
