//! `ease.context` JS bridge — exposes the storage context this tur view
//! instance represents, plus a `disconnect()` action for edit-mode views.
//!
//! For **edit-mode** views (one per existing storage row), the host stamps
//! the storage's `plugin_storage_id` (e.g. `"onedrive:abc123"`) into the
//! per-instance data slot at build time. [`PluginInstance`] wraps it as
//! `Option<String>`; `None` for create-mode setup views and for the
//! headless service instance.
//!
//! Plugin JS calls (via the `context` namespace object on the unified
//! `ease` module):
//! - `context.instance()` → `string | null` (the storage's plugin_storage_id).
//! - `context.mode()`     → `"create" | "edit"` (convenience for branching).
//! - `context.disconnect()` → remove the storage row this view represents
//!   (no-op in create mode). Host-side `EaseStorageHost.disconnect` finds
//!   the storage by `(pluginId, plugin_storage_id)` and invokes
//!   `storageRepository.pluginRemoveInstance(id)`; the resulting
//!   `pluginDisconnectedEvent` flow lets `EditStoragesPage` pop back.

use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use crate::plugin_runtime::{PluginId, PluginInstance};

/// Build the `FnEntry` table for the `context` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![
        ("instance", 0, instance as Ptr),
        ("mode", 0, mode as Ptr),
        ("disconnect", 0, disconnect as Ptr),
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

/// `context.instance()` → `string | null`.
fn instance(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let (_, inst) = resolve(args)?;
    match inst.0 {
        Some(s) => Ok(JsValue::from(js_string!(s.as_str()))),
        None => Ok(JsValue::null()),
    }
}

/// `context.mode()` → `"create" | "edit"`.
fn mode(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let (_, inst) = resolve(args)?;
    let s = match inst.0 {
        Some(_) => "edit",
        None => "create",
    };
    Ok(JsValue::from(js_string!(s)))
}

/// `context.disconnect()` — remove the storage row this view represents.
///
/// In create mode (`PluginInstance::None`) this is a no-op. In edit mode
/// it upcalls to the Kotlin host, which looks up the storage by
/// `(pluginId, plugin_storage_id)` and invokes
/// `storageRepository.pluginRemoveInstance(id)`; the resulting
/// `pluginDisconnectedEvent` lets the host-side UI pop back.
fn disconnect(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let (pid, inst) = resolve(args)?;
    let Some(instance) = inst.0 else {
        tracing::debug!("ease:context.disconnect ignored (create mode — no instance)");
        return Ok(JsValue::undefined());
    };

    #[cfg(target_os = "android")]
    {
        if let Err(e) = upcall_disconnect(pid.as_str(), &instance) {
            tracing::warn!("ease:context.disconnect upcall failed: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (pid, instance);
        tracing::debug!("ease:context.disconnect ignored (non-Android target)");
    }

    Ok(JsValue::undefined())
}

#[cfg(target_os = "android")]
fn upcall_disconnect(plugin_id: &str, instance: &str) -> Result<(), String> {
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
        "disconnect",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&pid_jstr), JValue::Object(&inst_jstr)],
    )
    .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(())
}
