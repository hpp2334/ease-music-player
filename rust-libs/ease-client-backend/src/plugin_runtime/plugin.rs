//! `EaseMusicPlugin` — the central plugin that registers the unified
//! `ease` JS module with the tur engine.
//!
//! `ease` exports four grouped namespace objects, each a `JsObject` whose
//! methods are ctx-bound native fns built via `bound_native` (tur's
//! standard "prepend the per-instance `TurInstanceContext` to args"
//! wrapper). Bridge fns then call `extract_js_ctx(args)?` + (for
//! storage/secret/oauth) `js_ctx.data::<PluginId>()?` to resolve the
//! calling plugin's identity from the per-instance data slot.
//!
//!     import { storage, secret, oauth, themes } from "ease";
//!     storage.get("key");              // ← plugin id resolved in Rust
//!     secret.put("refresh-token");
//!     oauth.start("onedrive", alias);
//!     themes.color("primary");
//!
//! Installed alongside `TurStdPlugin` / `TurAnimationPlugin` /
//! `TurClipboardPlugin` / `TurNetPlugin` (the standard tur set) by
//! [`super::plugin_jni::create_ease_plugin_engine`]. Carries no per-instance
//! state — every call resolves the active [`crate::BackendContext`] through
//! the [`crate::BACKEND_CONTEXT`] OnceLock at call time.

use boa_engine::object::JsObject;
use boa_engine::{js_string, JsValue};
use tur_engine::core::js_runtime::helpers::ConstEntry;
use tur_engine::core::js_runtime::module_loader::bound_native;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

use super::{context_bridge, oauth_bridge, secret_bridge, storage_bridge, themes_bridge};

pub struct EaseMusicPlugin;

impl Default for EaseMusicPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for EaseMusicPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // Clone the ctx value BEFORE mutably borrowing `boa` — the build
        // loop below needs both simultaneously.
        let js_ctx_value = ctx.js_ctx_value.clone();
        let boa = ctx.boa_mut();

        // Build the four grouped namespace objects. Each method is a
        // ctx-bound native fn (`bound_native` prepends the per-instance
        // `TurInstanceContext` to args, so the fn receives it as args[0]
        // and calls `extract_js_ctx` to pull it out).
        let storage_obj = build_namespace(boa, &js_ctx_value, storage_bridge::build_fns());
        let secret_obj = build_namespace(boa, &js_ctx_value, secret_bridge::build_fns());
        let oauth_obj = build_namespace(boa, &js_ctx_value, oauth_bridge::build_fns());
        let themes_obj = build_namespace(boa, &js_ctx_value, themes_bridge::build_fns());
        let context_obj = build_namespace(boa, &js_ctx_value, context_bridge::build_fns());

        let consts: Vec<ConstEntry> = vec![
            ("storage", JsValue::from(storage_obj)),
            ("secret", JsValue::from(secret_obj)),
            ("oauth", JsValue::from(oauth_obj)),
            ("themes", JsValue::from(themes_obj)),
            ("context", JsValue::from(context_obj)),
        ];

        ctx.register_module("ease", vec![], vec![], consts);

        tracing::info!(
            "EaseMusicPlugin registered ease (unified: storage + secret + oauth + themes + context)"
        );
        Ok(())
    }
}

/// Build a grouped namespace `JsObject` from a `FnEntry` table. Each entry
/// becomes a property on the object whose value is the ctx-bound native
/// fn — `obj.method(args...)` reaches the fn as `fn(ctx_value, args...)`,
/// and the fn uses `extract_js_ctx(args)` to recover the per-instance
/// context.
fn build_namespace(
    boa: &mut boa_engine::Context,
    js_ctx_value: &JsValue,
    fns: Vec<(&'static str, usize, tur_engine::core::js_runtime::helpers::Ptr)>,
) -> JsObject {
    let obj = JsObject::with_object_proto(boa.intrinsics());
    for (name, length, ptr) in fns {
        let method = bound_native(boa, js_ctx_value.clone(), ptr, length, name);
        let _ = obj.create_data_property(js_string!(name), JsValue::from(method), boa);
    }
    obj
}
