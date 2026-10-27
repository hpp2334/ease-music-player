//! `EaseMusicPlugin` — the central plugin that installs the `ease` host
//! rows with the tur engine.
//!
//! Two pkg extensions ride one plugin:
//!
//! - [`service_rows::install`] — the backend-bound service surface (`db`,
//!   `secret`, `oauth`, `themes`, `context`, `library`, `rpc.call`). Every
//!   scoped row resolves the calling plugin's identity from the
//!   per-instance data slot ([`PluginId`]) and the backend from the
//!   per-instance plugin state ([`PluginBackendCx`]) — never from a rut
//!   argument.
//! - [`util_rows::install`] — the identity-free utilities (`crypto`, `xml`,
//!   `net`, `time`).
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

use super::{service_rows, util_rows, PluginBackendCx};

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

        ctx.push_rut_ext(std::rc::Rc::new(service_rows::install));
        ctx.push_rut_ext(std::rc::Rc::new(util_rows::install));

        tracing::info!(
            "EaseMusicPlugin registered ease (db + secret + oauth + themes + context + library + rpc.call; utilities: crypto + xml + net + time)"
        );
        Ok(())
    }
}
