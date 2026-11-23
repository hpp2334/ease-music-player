//! `EaseMusicPlugin` — the central plugin that installs the `emp` host
//! surface with the tur engine.
//!
//! Two pkg extensions ride one plugin:
//!
//! - [`emp::install`] — the SDK surface: the `emp_host` host pkg (service
//!   rows with a leading handle opaque — the host-minted
//!   [`emp::PluginHandle`] the phase-1 boot lane delivers to
//!   `start(cx)`) + the `emp` class pkg offered beside it. The rpc
//!   bind-table rows mount through `ease_tur_rpc::install_rows` (the
//!   one-owner relocation seam); the `emp` `Rpc` class covers them.
//! - [`third::install`] — the identity-free util libs (`crypto`, `xml`,
//!   `net`, `time`) as standard rut pkgs from `plugins/3rd/`.
//!
//! The rpc bind rows' sealed-handler table is stamped by
//! [`ease_tur_rpc::TurRpcBindPlugin`], registered alongside this plugin by
//! [`super::plugin_jni`] so host tests can load modules without the
//! backend binding.
//!
//! ```rut
//! import { Db } from emp;
//!
//! entry fn start(cx: opaque) {
//!     const db = Db(cx);   // identity rides the host-minted handle
//!     ...
//! }
//! ```

use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

use super::{emp, third, PluginBackendCx};

/// The `emp` host-row plugin. Carries the
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

        // The SDK surface: the `emp` classes over the `emp_host` rows
        // (including the rpc bind table) + the 3rd-party util libs.
        ctx.push_rut_ext(std::rc::Rc::new(emp::install));
        ctx.push_rut_ext(std::rc::Rc::new(third::install));

        tracing::info!(
            "EaseMusicPlugin registered emp (emp_host incl. the rpc bind table + the emp class SDK) + the 3rd libs (crypto + xml + net + time)"
        );
        Ok(())
    }
}
