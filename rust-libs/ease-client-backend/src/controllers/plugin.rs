use std::sync::Arc;

use ease_client_schema::{
    PluginKvCountEntry, PluginKvEntry, PluginKvKeyInfo, PluginKvMultiEntry,
};
use ease_client_tokio::tokio_runtime;

use crate::{error::BResult, Backend};

// ============================================================================
// Plugin KV storage — UniFFI-exported controllers.
//
// Two storage modes share the same `(plugin_id, key)` registry:
//   * Single — overwrite-mode, one value per key.
//   * Multi  — append-only, many values per key (event log).
//
// All multi-key operations (get_multi / set_multi / append_multi /
// count_multi / delete_multi) resolve keys via indexed `IN (...)` lookups
// in a single round-trip.
// ============================================================================

// ---------------------------------------------------------------------------
// Single-value (overwrite) operations
// ---------------------------------------------------------------------------

#[uniffi::export]
pub async fn ct_plugin_kv_single_set(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
    value: String,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_single_set(&plugin_id, &key, &value).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_single_set_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    entries: Vec<PluginKvEntry>,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_single_set_multi(&plugin_id, entries).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_single_get(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
) -> BResult<Option<String>> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_single_get(&plugin_id, &key).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_single_get_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    keys: Vec<String>,
) -> BResult<Vec<PluginKvEntry>> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_single_get_multi(&plugin_id, keys).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_single_delete(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_single_delete(&plugin_id, &key).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_single_delete_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    keys: Vec<String>,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_single_delete_multi(&plugin_id, keys).await })
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Multi-value (append-only) operations
// ---------------------------------------------------------------------------

#[uniffi::export]
pub async fn ct_plugin_kv_multi_append(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
    value: String,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_append(&plugin_id, &key, &value).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_append_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    entries: Vec<PluginKvEntry>,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_append_multi(&plugin_id, entries).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_get_all(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
) -> BResult<Vec<String>> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_get_all(&plugin_id, &key).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_get_all_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    keys: Vec<String>,
) -> BResult<Vec<PluginKvMultiEntry>> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_get_all_multi(&plugin_id, keys).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_count(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
) -> BResult<u64> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_count(&plugin_id, &key).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_count_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    keys: Vec<String>,
) -> BResult<Vec<PluginKvCountEntry>> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_count_multi(&plugin_id, keys).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_delete(
    cx: Arc<Backend>,
    plugin_id: String,
    key: String,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_delete(&plugin_id, &key).await })
        .await
        .unwrap()
}

#[uniffi::export]
pub async fn ct_plugin_kv_multi_delete_multi(
    cx: Arc<Backend>,
    plugin_id: String,
    keys: Vec<String>,
) -> BResult<()> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_multi_delete_multi(&plugin_id, keys).await })
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Key listing (works across both modes)
// ---------------------------------------------------------------------------

#[uniffi::export]
pub async fn ct_plugin_kv_list_keys(
    cx: Arc<Backend>,
    plugin_id: String,
    prefix: String,
) -> BResult<Vec<PluginKvKeyInfo>> {
    let db = cx.get_context().database_server().clone();
    tokio_runtime()
        .handle()
        .spawn(async move { db.plugin_kv_list_keys(&plugin_id, &prefix).await })
        .await
        .unwrap()
}
