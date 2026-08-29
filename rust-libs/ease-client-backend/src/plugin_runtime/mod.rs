//! tur plugin runtime — in-memory bridge between the ease backend and the
//! tur engine.
//!
//! This module wires the engine (linked as an rlib into
//! `libease_client_backend.so`) to the ease backend's services via the
//! process-wide [`crate::BACKEND_CONTEXT`] OnceLock. The bridge exposes
//! one synthetic JS module to plugins:
//!
//! - `ease` — unified host module exporting four grouped namespace objects:
//!   `db`, `secret`, `oauth`, `themes`. Each method is ctx-bound
//!   (`extract_js_ctx`) so bridge fns can resolve the calling plugin's
//!   identity from the per-instance data slot — never from a JS argument.
//!
//! Per-instance identity: the Kotlin host stamps a [`PluginId`] into each
//! tur instance at build time via `TurAppBuilder::instance_data(|cx|
//! cx.define::<PluginId>(...))` (see `plugin_jni.rs`). Bridge fns read it
//! back via `js_ctx.data::<PluginId>()` to enforce per-plugin scoping in
//! SQLite + the secret store. The value is JS-unforgeable — it never crosses
//! the JS↔Rust boundary as an argument.
//!
//! Other tur modules (`tur:std`, `tur:animation`, `tur:net`) are
//! registered by the standard plugin set installed alongside
//! [`EaseMusicPlugin`] in `plugin_jni::create_ease_plugin_engine`.

pub mod context_bridge;
pub mod db_bridge;
pub mod host_cache;
pub mod oauth_bridge;
pub mod plugin;
pub mod plugin_jni;
pub mod rpc_bridge;
pub mod secret_bridge;
pub mod themes_bridge;
pub mod webapi;

pub use plugin::EaseMusicPlugin;

#[cfg(target_os = "android")]
pub(crate) use plugin_jni::read_asset_bytes;

/// Per-instance plugin identity, stamped into the tur engine's
/// `instance_data` slot at build time (via `TurAppBuilder::instance_data`)
/// and read back by `ease:*` bridge fns to resolve the calling plugin.
///
/// The wrapped `String` is the plugin's stable identifier (e.g.
/// `"com.ease.onedrive"`). The field is private so external code must go
/// through [`Self::as_str`] / [`Self::as_ref`] — JS itself never sees this
/// value (no accessor is exposed to JS), so identity stays JS-unforgeable.
#[derive(Debug, Clone)]
pub struct PluginId(String);

impl PluginId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PluginId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Per-instance storage handle, stamped into the tur engine's
/// `instance_data` slot for **edit-mode** plugin views (one per storage
/// row the user is editing). The wrapped `String` is the storage's
/// `plugin_storage_id` (e.g. `"onedrive:abc123"`).
///
/// `None` for create-mode setup views (no storage row yet) and for the
/// headless service instance (which serves *all* instances, not one).
/// The `ease.context` namespace surfaces this to JS as the reactive
/// `storageId$` const (`null` = create, a real id = edit), minted in
/// [`plugin::EaseMusicPlugin::register`].
#[derive(Debug, Clone)]
pub struct PluginInstance(pub Option<String>);
