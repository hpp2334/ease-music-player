//! `EaseMusicPlugin` — the central plugin that registers `ease:*` JS
//! modules with the tur engine.
//!
//! Installed alongside `TurStdPlugin` / `TurAnimationPlugin` /
//! `TurClipboardPlugin` / `TurNetPlugin` (the standard tur set) by
//! [`super::plugin_jni::create_ease_plugin_engine`]. Carries no
//! per-instance state — every call resolves the active
//! [`crate::BackendContext`] through the [`crate::BACKEND_CONTEXT`]
//! OnceLock at call time.

use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

use super::{secret_bridge, storage_bridge};

pub struct EaseMusicPlugin;

impl Default for EaseMusicPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for EaseMusicPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // Register `ease:storage` as a host module (ctx-free). All bridge
        // fns reach the backend via BACKEND_CONTEXT.
        let exports: Vec<(String, boa_engine::NativeFunction, usize)> = storage_bridge::build_host_fns()
            .into_iter()
            .map(|(n, f, l)| (n.to_string(), f, l))
            .collect();
        ctx.register_host_module("ease:storage", exports);

        // `ease:secret` — owner-scoped secret access (get/put/remove). The
        // SecretStore enforces `scope == "plugin:<pluginId>"`.
        let secret_exports: Vec<(String, boa_engine::NativeFunction, usize)> =
            secret_bridge::build_host_fns()
                .into_iter()
                .map(|(n, f, l)| (n.to_string(), f, l))
                .collect();
        ctx.register_host_module("ease:secret", secret_exports);

        // Export KIND_SINGLE / KIND_MULTI as consts so plugin JS does not
        // hardcode the integer discriminants. We expose them by registering
        // a second tiny host module `ease:storage/constants` — tur's
        // `register_host_module` only accepts functions, so we synthesize
        // getters. (Two trivial fns, near-zero overhead.)
        ctx.register_host_module(
            "ease:storage/constants",
            vec![
                (
                    "KIND_SINGLE".to_string(),
                    boa_engine::NativeFunction::from_copy_closure(|_, _, _| {
                        Ok(boa_engine::JsValue::from(0))
                    }),
                    0,
                ),
                (
                    "KIND_MULTI".to_string(),
                    boa_engine::NativeFunction::from_copy_closure(|_, _, _| {
                        Ok(boa_engine::JsValue::from(1))
                    }),
                    0,
                ),
            ],
        );

        tracing::info!("EaseMusicPlugin registered ease:storage + ease:secret");
        Ok(())
    }
}
