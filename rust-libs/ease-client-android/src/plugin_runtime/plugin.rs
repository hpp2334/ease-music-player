//! `EaseMusicPlugin` — the central plugin that installs the `ease` host
//! rows with the tur engine.
//!
//! Four pkg extensions ride one plugin (the old surface stays alive
//! beside the new — D5, the un-migrated plugins compile against it
//! until phase 5):
//!
//! - [`service_rows::install`] — the backend-bound OLD surface (`db`,
//!   `secret`, `oauth`, `themes`, `context`, `library`, `rpc.call`).
//!   Every scoped row resolves the calling plugin's identity from the
//!   per-instance data slot ([`PluginId`]) and the backend from the
//!   per-instance plugin state ([`PluginBackendCx`]) — never from a rut
//!   argument.
//! - [`util_rows::install`] — the OLD identity-free utilities
//!   (`crypto`, `xml`, `net`, `time`).
//! - [`emp::install`] — the NEW SDK surface: the `emp_host` host pkg
//!   (service rows with a leading handle opaque — the host-minted
//!   [`emp::PluginHandle`] the phase-1 boot lane delivers to
//!   `start(cx)`) + the `emp` class pkg offered beside it.
//! - [`third::install`] — the NEW identity-free util libs (`crypto`,
//!   `xml`, `net`, `time`) as standard rut pkgs from `plugins/3rd/`.
//!
//! The `ease.rpc` bus rows (`register` / `subscribe_event` /
//! `take_request`) live in [`ease_tur_rpc::TurRpcBusPlugin`], registered
//! alongside this plugin by [`super::plugin_jni`] so host tests can load
//! them without the backend binding.
//!
//! ```rut
//! import { single_get, single_set } from tur_host.ease.db;
//! import { color } from tur_host.ease.themes;
//!
//! entry fn start() -> opaque {
//!     // identity is resolved host-side — no plugin id anywhere:
//!     let v = single_get("config");
//!     ...
//! }
//! ```

use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

use super::{emp, service_rows, third, util_rows, PluginBackendCx};

/// The `ease` host-row plugin. Carries the
/// [`ease_client_backend::ctx::BackendContext`] this engine is bound to
/// (resolved from the backend handle at `createRuntime`) and stamps it
/// into every instance's plugin state at register time.
pub struct EaseMusicPlugin {
    cx: ease_client_backend::ctx::BackendContext,
}

impl EaseMusicPlugin {
    /// Bind the plugin to `cx`. The context must outlive every instance
    /// spawned from the runtime — it is held by a cheap clone here and by
    /// each instance's [`PluginBackendCx`] plugin state.
    pub fn new(cx: ease_client_backend::ctx::BackendContext) -> Self {
        Self { cx }
    }
}

impl Plugin for EaseMusicPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
        // Bind this instance to the plugin's backend context BEFORE
        // anything else — every backend-bound row reads it back via
        // `backend_cx`. `define_plugin_state` is register-phase only and
        // panics on a duplicate type, so a double registration fails fast
        // instead of silently rebinding.
        ctx.define_plugin_state(std::rc::Rc::new(PluginBackendCx(self.cx.clone())));

        // The OLD surface stays alive (D5 — un-migrated plugins compile
        // against it until phase 5).
        ctx.push_rut_ext(std::rc::Rc::new(service_rows::install));
        ctx.push_rut_ext(std::rc::Rc::new(util_rows::install));
        // The NEW surface (phase 2): the `emp` SDK + the 3rd-party util
        // libs, ADDED beside the old rows.
        ctx.push_rut_ext(std::rc::Rc::new(emp::install));
        ctx.push_rut_ext(std::rc::Rc::new(third::install));

        tracing::info!(
            "EaseMusicPlugin registered ease (db + secret + oauth + themes + context + library + rpc.call; utilities: crypto + xml + net + time) + emp (emp_host + the emp class SDK) + the 3rd libs (crypto + xml + net + time)"
        );
        Ok(())
    }
}
