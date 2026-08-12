//! `ease.themes` JS bridge — read-only access to the host app's Material 3
//! theme, so plugin *setup views* (and full plugin pages) can inherit the
//! app's colors instead of hardcoding their own.
//!
//! Plugin JS calls (via the `themes` namespace object on the unified
//! `ease` module):
//! - `themes.color("primary")` → `"#RRGGBBAA"` (or `""` if unknown).
//! - `themes.isDark()` → `boolean`.
//!
//! Kotlin is the source of truth: the app theme pushes the resolved
//! `ColorScheme` into `EaseThemesHost` (a `@Volatile` map) on every
//! recomposition; the bridge upcalls `EaseThemesHost.getColor(name)` /
//! `EaseThemesHost.isDark()` via JNI (the engine thread is already
//! attached to the JVM). Non-Android targets compile as stubs returning
//! empty/false.
//!
//! Bridge fns are ctx-bound for consistency with the rest of the `ease`
//! module, but do not read the per-instance `PluginId` — theme state is
//! app-global, not per-plugin.

use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

/// Build the `FnEntry` table for the `themes` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![
        ("color", 1, color as Ptr),
        ("isDark", 0, is_dark as Ptr),
    ]
}

fn require_string(args: &[JsValue], idx: usize) -> JsResult<String> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "ease:themes: missing required string argument at index {idx}"
        ))));
    }
    let s = v.as_string().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:themes: expected string at index {idx}"
        )))
    })?;
    Ok(s.to_std_string_escaped())
}

/// `themes.color(name)` → `"#RRGGBBAA"` hex string (or `""`).
fn color(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    // Ctx-bound (consistent with the rest of `ease`) — the per-instance
    // context is the first arg; the user-facing `name` is at index 1.
    let _ = extract_js_ctx(args)?;
    let name = require_string(args, 1)?;

    #[cfg(target_os = "android")]
    let hex = upcall_get_color(&name).unwrap_or_default();
    #[cfg(not(target_os = "android"))]
    let hex = {
        let _ = &name;
        String::new()
    };

    Ok(JsValue::from(js_string!(hex.as_str())))
}

/// `themes.isDark()` → `boolean`.
fn is_dark(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    #[cfg(target_os = "android")]
    let dark = upcall_is_dark().unwrap_or(false);
    #[cfg(not(target_os = "android"))]
    let dark = false;

    Ok(JsValue::from(dark))
}

#[cfg(target_os = "android")]
fn upcall_get_color(name: &str) -> Result<String, String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = super::host_cache::theme_host_class()
        .ok_or("theme host class not cached")?;
    let class = unsafe { JClass::from_raw(raw_class) };
    let name_jstr = env
        .new_string(name)
        .map_err(|e| format!("new_string name: {e}"))?;
    let ret = env
        .call_static_method(
            class,
            "getColor",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&name_jstr)],
        )
        .map_err(|e| format!("call_static_method: {e}"))?;
    let jstr = ret
        .l()
        .map_err(|e| format!("ret.l: {e}"))?
        .into_raw();
    let jstr = unsafe { jni::objects::JString::from_raw(jstr) };
    let s: String = env
        .get_string(&jstr)
        .map_err(|e| format!("get_string: {e}"))?
        .into();
    Ok(s)
}

#[cfg(target_os = "android")]
fn upcall_is_dark() -> Result<bool, String> {
    use jni::objects::JClass;

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = super::host_cache::theme_host_class()
        .ok_or("theme host class not cached")?;
    let class = unsafe { JClass::from_raw(raw_class) };
    let ret = env
        .call_static_method(class, "isDark", "()Z", &[])
        .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(ret.z().map_err(|e| format!("ret.z: {e}"))?)
}
