//! `ease.context` JS bridge — per-instance storage identity + lifecycle hooks
//! for plugin storage views.
//!
//! Exposes (on the `context` namespace object of the unified `ease` module):
//! - `notifyChange()` → upcall asking the host to reload its storage list
//!   (so a kv-side edit like an alias rename, or a removal, is reflected in
//!   the dashboard). Identity is resolved from the per-instance data slot.
//! - `removeStorage(pluginStorageId)` → delete the host storage row for
//!   `(pluginId, pluginStorageId)`. Called by a plugin backend (e.g.
//!   `onedrive:removeInstance`) after it wipes its own kv + secret, so the
//!   backend fully owns the disconnect: its own cleanup + asking the host to
//!   drop the registry row.
//!
//! The reactive `storageId$` (a `Readable<string|null>` seeded from
//! [`PluginInstance`]) is minted in [`super::plugin`] and attached to the
//! same `context` namespace object as a data property; JS reads it via
//! `get(context.storageId$)`. `null` = create-mode setup view; a real
//! `plugin_storage_id` = edit view for that storage.

use boa_engine::{JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use crate::error::BResult;
use crate::plugin_runtime::{PluginId, PluginInstance};

/// Build the `FnEntry` table for the `context` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![
        ("notifyChange", 0, notify_change as Ptr),
        ("removeStorage", 1, remove_storage as Ptr),
    ]
}

/// Resolve `(PluginId, PluginInstance)` from the per-instance data slot.
fn resolve(args: &[JsValue]) -> JsResult<(PluginId, PluginInstance)> {
    let js_ctx = extract_js_ctx(args)?;
    let pid = js_ctx.data::<PluginId>().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "ease:context: no plugin context bound to this instance",
        ))
    })?;
    let instance = js_ctx
        .data::<PluginInstance>()
        .unwrap_or_else(|| PluginInstance(None));
    Ok((pid, instance))
}

/// Pull the calling plugin's identity (PluginId only).
fn plugin_id(args: &[JsValue]) -> JsResult<PluginId> {
    Ok(resolve(args)?.0)
}

fn require_string(args: &[JsValue], idx: usize) -> JsResult<String> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "ease:context: missing required string argument at index {idx}"
        ))));
    }
    let s = v.as_string().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:context: expected string at index {idx}"
        )))
    })?;
    Ok(s.to_std_string_escaped())
}

fn backend_ctx() -> JsResult<&'static std::sync::Arc<crate::ctx::BackendContext>> {
    crate::BACKEND_CONTEXT.get().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "ease:context: backend not initialized (BACKEND_CONTEXT is unset)",
        ))
    })
}

fn map_bresult<R>(description: &str, result: BResult<R>) -> JsResult<R> {
    result.map_err(|e| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:context {description} failed: {e:?}"
        )))
    })
}

/// `context.notifyChange()` → upcall to the Kotlin host, which reloads its
/// storage list so kv-side changes (alias rename) or removals propagate to
/// the dashboard + edit page.
fn notify_change(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let (pid, inst) = resolve(args)?;
    let instance_str = inst.0.unwrap_or_default();

    #[cfg(target_os = "android")]
    {
        if let Err(e) = upcall_notify_change(pid.as_str(), &instance_str) {
            tracing::warn!("ease:context.notifyChange upcall failed: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (pid, instance_str);
        tracing::debug!("ease:context.notifyChange ignored (non-Android target)");
    }

    Ok(JsValue::undefined())
}

/// `context.removeStorage(pluginStorageId)` → delete the host storage row
/// for `(pluginId, pluginStorageId)`. Called by a plugin backend after it
/// wipes its own kv + secret, completing the disconnect. No-op if no row
/// matches (already removed).
fn remove_storage(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    use ease_client_schema::StorageId;

    let pid = plugin_id(args)?;
    let plugin_storage_id = require_string(args, 1)?;
    let cx = backend_ctx()?;
    let pid_str = pid.as_str().to_string();

    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        let rows = cx.database_server().load_all_storage_rows().await?;
        let id = rows
            .into_iter()
            .find(|r| {
                r.plugin_id.as_deref() == Some(pid_str.as_str())
                    && r.plugin_storage_id.as_deref() == Some(plugin_storage_id.as_str())
            })
            .map(|r| StorageId::wrap(r.id));
        if let Some(id) = id {
            crate::services::storage::remove_storage(&**cx, id).await?;
        }
        Ok::<_, crate::error::BError>(())
    });
    map_bresult("removeStorage", result)?;

    Ok(JsValue::undefined())
}

#[cfg(target_os = "android")]
fn upcall_notify_change(plugin_id: &str, instance: &str) -> Result<(), String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = super::host_cache::storage_host_class()
        .ok_or("storage host class not cached")?;
    let class = unsafe { JClass::from_raw(raw_class) };
    let pid_jstr = env
        .new_string(plugin_id)
        .map_err(|e| format!("new_string plugin_id: {e}"))?;
    let inst_jstr = env
        .new_string(instance)
        .map_err(|e| format!("new_string instance: {e}"))?;
    env.call_static_method(
        class,
        "notifyChange",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&pid_jstr), JValue::Object(&inst_jstr)],
    )
    .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(())
}
