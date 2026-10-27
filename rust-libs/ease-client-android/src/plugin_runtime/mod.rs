//! tur plugin runtime — the rut-side bridge between the ease backend and
//! the tur engine.
//!
//! This module wires the engine (linked as an rlib into
//! `libease_client_android.so`) to the ease backend's services. The backend
//! binding is **instance-attached**: `createRuntime` resolves the
//! [`ease_client_backend::ctx::BackendContext`] from the backend handle the
//! Kotlin host passes in, [`plugin::EaseMusicPlugin`] carries it and stamps
//! it into every instance's plugin state at register time
//! ([`PluginBackendCx`]), and the host rows read it back from the instance —
//! no process-wide singleton whose entry could outlive a destroyed backend.
//!
//! The bridge exposes one host pkg to rut modules — the `ease` rows:
//!
//! - `ease.db` / `ease.secret` / `ease.oauth` / `ease.themes` /
//!   `ease.context` / `ease.library` — the host services (KV, secrets,
//!   OAuth flows, theme colors, storage lifecycle, read-only library).
//! - `ease.rpc` — the plugin RPC bus ([`ease_tur_rpc::TurRpcBusPlugin`]:
//!   `register` / `subscribe_event` / `take_request`) plus `call` (a view
//!   reaching its own backend).
//! - `ease.crypto` / `ease.xml` / `ease.net` / `ease.time` — the utility
//!   surface that replaced the old npm plugin deps (md5 / base64 / SHA-2 /
//!   HMAC / UUID, XML, full-featured HTTP, HTTP dates).
//!
//! Rows are **typed** (`rut_core::types`): scalars cross as str/u64/i64/
//! f64/bool, binary as bytes, composites as JSON strings, host objects as
//! opaques. Per-instance identity: the host stamps a [`PluginId`] (and for
//! edit-mode views a [`PluginInstance`]) into the engine's `instance_data`
//! at build time (`TurAppBuilder::instance_data`) — rows resolve the calling
//! plugin through it, never from a rut argument. The value is script-
//! unforgeable.
//!
//! There is no JS in the plugin system: modules are rut sources loaded by
//! opaque `ModuleSourceRegistry` handles, and every host entry point is a
//! typed row.

pub mod engine;
pub mod fonts;
pub mod host_cache;
pub mod plugin;
pub mod plugin_jni;
pub mod service_rows;
pub mod util_rows;

pub use plugin::EaseMusicPlugin;

use ease_tur_rpc::RpcBusCell;

/// Per-instance plugin identity, stamped into the tur engine's
/// `instance_data` slot at build time (via `TurAppBuilder::instance_data`)
/// and read back by `ease:*` rows to resolve the calling plugin.
///
/// The wrapped `String` is the plugin's stable identifier (e.g.
/// `"com.ease.onedrive"`). The field is private so external code must go
/// through [`Self::as_str`] / [`Self::as_ref`] — rut itself never sees this
/// value (no row returns it), so identity stays script-unforgeable.
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
/// The `ease.context.storage_id()` row surfaces this to rut (`?str`:
/// `nil` = create, a real id = edit).
#[derive(Debug, Clone)]
pub struct PluginInstance(pub Option<String>);

/// The backend context an engine instance is bound to, captured at
/// `createRuntime` (from the backend handle the Kotlin host passes) and
/// defined as per-instance plugin state by
/// [`plugin::EaseMusicPlugin::register`]. Rows resolve it via
/// [`backend_cx`] — there is no process-wide backend singleton.
///
/// [`ease_client_backend::ctx::BackendContext`] is an `Arc`-backed cheap
/// clone, so rows clone it out of the `Rc`-boxed plugin state without
/// lifetime gymnastics.
pub struct PluginBackendCx(pub ease_client_backend::ctx::BackendContext);

/// Resolve the [`ease_client_backend::ctx::BackendContext`] bound to the
/// calling plugin instance ([`PluginBackendCx`]). `ns` names the `ease`
/// row family for the error message. Fails loudly when the engine was
/// built without a backend binding.
pub(crate) fn backend_cx(
    ns: &str,
    handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>,
) -> Result<ease_client_backend::ctx::BackendContext, rut_vm::Trap> {
    handles
        .inst
        .plugin_state::<PluginBackendCx>()
        .map(|cx| cx.0.clone())
        .ok_or_else(|| {
            rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                format!(
                    "ease:{ns}: backend context not bound to this instance \
                     (engine built without a backend handle)"
                ),
            )
        })
}

/// Resolve the calling plugin's [`PluginId`] (the instance-data slot).
pub(crate) fn plugin_id(
    ns: &str,
    handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>,
) -> Result<PluginId, rut_vm::Trap> {
    handles.inst.data::<PluginId>().ok_or_else(|| {
        rut_vm::Trap::new(
            rut_vm::TrapKind::Invalid,
            format!("ease:{ns}: no plugin context bound to this instance"),
        )
    })
}

/// Resolve the per-instance RPC bus cell (see [`RpcBusCell`]) — stamped by
/// the embedder for headless backend instances; views fall back to the
/// per-instance cell [`ease_tur_rpc::TurRpcBusPlugin`] mints at register.
pub(crate) fn rpc_bus_cell(
    handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>,
) -> RpcBusCell {
    handles
        .inst
        .data::<RpcBusCell>()
        .or_else(|| handles.inst.plugin_state::<RpcBusCell>().map(|c| (*c).clone()))
        .expect("ease:rpc: no RpcBusCell in instance data or plugin state")
}
