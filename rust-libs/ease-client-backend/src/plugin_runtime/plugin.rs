//! `EaseMusicPlugin` — the central plugin that registers the unified
//! `ease` JS module with the tur engine.
//!
//! `ease` exports grouped namespace objects, each a `JsObject` whose
//! methods are ctx-bound native fns built via `bound_native` (tur's
//! standard "prepend the per-instance `TurInstanceContext` to args"
//! wrapper). Bridge fns then call `extract_js_ctx(args)?` + (for
//! db/secret/oauth/rpc) `js_ctx.data::<PluginId>()?` to resolve the
//! calling plugin's identity from the per-instance data slot.
//!
//! ```js
//! import { db, secret, oauth, themes, context, rpc } from "ease";
//! db.singleGet("key");              // ← plugin id resolved in Rust
//! secret.put("refresh-token");
//! oauth.start("onedrive", alias);
//! themes.color("primary");
//! rpc.call("onedrive:list", { ... });   // view → its backend
//! store.get(context.storageId$);        // null = create, id = edit
//! ```
//!
//! The `context` namespace additionally carries `storageId$` — a
//! `Readable<string|null>` source minted per-instance from
//! [`PluginInstance`] via [`PluginRegisterContext::reactive`] (tur #189),
//! seeded before this `register` runs (instance_data is populated first).
//!
//! Installed alongside `TurStdPlugin` / `TurAnimationPlugin` /
//! `TurClipboardPlugin` / `TurNetPlugin` (the standard tur set) by
//! [`super::plugin_jni::create_ease_plugin_engine`]. Carries no per-instance
//! state — every call resolves the active [`crate::BackendContext`] through
//! the [`crate::BACKEND_CONTEXT`] OnceLock at call time.

use boa_engine::object::JsObject;
use boa_engine::{js_string, JsValue};
use tur_engine::core::edgy::reactive::ReactiveBridgeStore;
use tur_engine::core::js_runtime::helpers::ConstEntry;
use tur_engine::core::js_runtime::js_value::IntoJs;
use tur_engine::core::js_runtime::module_loader::bound_native;
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

use super::{context_bridge, db_bridge, oauth_bridge, rpc_bridge, secret_bridge, themes_bridge};
use super::PluginInstance;

pub struct EaseMusicPlugin;

impl Default for EaseMusicPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for EaseMusicPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext<'_>) -> Result<(), TurError> {
        // Read the per-instance storage identity + mint the `storageId$`
        // source BEFORE borrowing `boa_mut()` — `ctx.reactive()` and
        // `ctx.js_ctx.data::<PluginInstance>()` are shared borrows, while
        // `into_js` needs the mutable boa borrow. The source is seeded once
        // with the instance's `plugin_storage_id` (null for create-mode
        // views + headless backends); it never changes for the instance
        // lifetime, so a `source` (not `derived`) is correct. Per-instance
        // isolation is automatic: `register` re-runs in each instance, whose
        // values materialize into that instance's single engine-created
        // store (tur #207), seeded from its own `PluginInstance`.
        let instance_id = ctx
            .js_ctx()
            .data::<PluginInstance>()
            .and_then(|i| i.0);
        let bridge: ReactiveBridgeStore = ctx.reactive();
        let id_value: JsValue = match &instance_id {
            Some(s) => JsValue::from(js_string!(s.as_str())),
            None => JsValue::null(),
        };
        let storage_id_src = bridge.decl_source::<JsValue>(id_value);

        // Clone the ctx value BEFORE mutably borrowing `boa` — the build
        // loop below needs both simultaneously.
        let js_ctx_value = ctx.js_ctx_value.clone();
        let boa = ctx.boa_mut();
        let storage_id_js = storage_id_src.into_js(boa);

        // Build the grouped namespace objects. Each method is a ctx-bound
        // native fn (`bound_native` prepends the per-instance
        // `TurInstanceContext` to args, so the fn receives it as args[0]
        // and calls `extract_js_ctx` to pull it out).
        let db_obj = build_namespace(boa, &js_ctx_value, db_bridge::build_fns());
        let secret_obj = build_namespace(boa, &js_ctx_value, secret_bridge::build_fns());
        let oauth_obj = build_namespace(boa, &js_ctx_value, oauth_bridge::build_fns());
        let themes_obj = build_namespace(boa, &js_ctx_value, themes_bridge::build_fns());
        let rpc_obj = build_namespace(boa, &js_ctx_value, rpc_bridge::build_fns());
        let context_obj = build_namespace(boa, &js_ctx_value, context_bridge::build_fns());

        // Attach the per-instance `storageId$` readable to the context
        // namespace object — JS reads it via `get(context.storageId$)`.
        let _ = context_obj.create_data_property(
            js_string!("storageId$"),
            storage_id_js,
            boa,
        );

        let consts: Vec<ConstEntry> = vec![
            ("db", JsValue::from(db_obj)),
            ("secret", JsValue::from(secret_obj)),
            ("oauth", JsValue::from(oauth_obj)),
            ("themes", JsValue::from(themes_obj)),
            ("rpc", JsValue::from(rpc_obj)),
            ("context", JsValue::from(context_obj)),
        ];

        ctx.register_module("ease", vec![], vec![], consts);

        tracing::info!(
            "EaseMusicPlugin registered ease (unified: db + secret + oauth + themes + rpc + context)"
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
