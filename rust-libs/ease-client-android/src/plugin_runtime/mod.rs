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
//! The host surface is the phase-2 `emp` SDK (phase 5 deleted the old
//! `tur_host::ease` rows — there is one surface):
//!
//! - `emp_host` + `emp` ([`emp`]) — the SDK: the host rows carry a
//!   leading handle opaque (the host-minted [`emp::PluginHandle`],
//!   delivered to `entry fn start(cx: opaque)` through the phase-1
//!   boot lane), and the `emp` classes (`Db` / `Secret` / `OAuth` /
//!   `Theme` / `Context` / `Library` / `Rpc`) wrap them one forwarded
//!   call per member. The rpc bind table (`rpc.bind` / `rpc.handler`,
//!   bodies in `ease-tur-rpc`) is part of the one `emp_host` mount.
//! - `crypto` / `xml` / `net` / `time` ([`third`]) — the identity-free
//!   utilities as standard rut pkg folders (`plugins/3rd/`), offered
//!   into the compile session by `include_str!` (one source of truth
//!   with what a `rut pack` closure would embed).
//!
//! Rows are **typed** (`rut_core::types`): scalars cross as str/u64/i64/
//! f64/bool, binary as bytes, composites as JSON strings, host objects as
//! opaques. Per-instance identity: the host stamps a [`PluginId`] (and for
//! edit-mode views a [`PluginInstance`]) plus the minted [`emp::PluginHandle`]
//! into the engine's `instance_data` at build time
//! (`TurAppBuilder::instance_data`) — the `emp_host` rows decode the
//! calling plugin from the handle against the instance's own stamp, never
//! from a rut argument. Both stamps are script-unforgeable.
//!
//! There is no JS in the plugin system: modules are rut sources loaded by
//! opaque `ModuleSourceRegistry` handles, and every host entry point is a
//! typed row.

pub mod emp;
pub mod engine;
pub mod fonts;
pub mod host_cache;
pub mod load;
pub mod plugin;
pub mod plugin_jni;
pub mod service_rows;
pub mod third;

pub use plugin::EaseMusicPlugin;

/// Per-instance plugin identity, stamped into the tur engine's
/// `instance_data` slot at build time (via `TurAppBuilder::instance_data`)
/// and minted into the [`emp::PluginHandle`] the `emp_host` rows decode.
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
/// The `emp` `Context.storage_id()` row surfaces this to rut (`?str`:
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
/// calling plugin instance ([`PluginBackendCx`]). `ns` names the `emp`
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
                    "emp:{ns}: backend context not bound to this instance \
                     (engine built without a backend handle)"
                ),
            )
        })
}
