//! `ease.oauth` JS bridge — fire-and-forget OAuth trigger for plugin
//! *setup views* (tur-rendered config forms).
//!
//! When a plugin's setup view (e.g. the OneDrive "Connect your account"
//! button) needs to start the OAuth dance, it calls
//! `ease.oauth.start(provider, alias?)`. The bridge reads the calling
//! plugin's identity from the per-instance data slot (no JS arg) and
//! forwards `(pluginId, provider, alias)` to the Kotlin host
//! (`EaseOauthHost.startOauth`) via a JNI static-method upcall on the JVM
//! the engine thread is already attached to. Kotlin then fetches the
//! provider's authorize URL via the headless service RPC, stashes the
//! `(pluginId, provider, alias)` triple, and opens the system browser;
//! the `easem://oauth2redirect` callback in `MainActivity` completes the
//! exchange.
//!
//! This is a one-way trigger — it returns immediately; the redirect lands
//! asynchronously. Non-Android targets compile as a no-op (host `cargo
//! check` / UniFFI codegen only).
//!
//! The `JavaVM` is obtained from `ndk_context` (initialized once by
//! `EaseMusicPlayerApplication.nativeInitAndroidContext` for cpal's AAudio
//! backend) rather than re-cached here.

use boa_engine::{JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use crate::plugin_runtime::PluginId;

/// Build the `FnEntry` table for the `oauth` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![("start", 2, start as Ptr)]
}

fn plugin_id(args: &[JsValue]) -> JsResult<PluginId> {
    let js_ctx = extract_js_ctx(args)?;
    js_ctx.data::<PluginId>().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("ease:oauth: no plugin context bound to this instance"),
        )
    })
}

fn require_string(args: &[JsValue], idx: usize) -> JsResult<String> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "ease:oauth: missing required string argument at index {idx}"
        ))));
    }
    let s = v.as_string().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message(format!("ease:oauth: expected string at index {idx}")),
        )
    })?;
    Ok(s.to_std_string_escaped())
}

/// `ease.oauth.start(provider, alias?)` — fire the OAuth flow on the Kotlin
/// host. Returns immediately (the redirect completes asynchronously).
///
/// The calling plugin's identity (from the per-instance data slot) is
/// validated purely to gate `ease.*` calls to plugin-bound instances —
/// `provider` itself is the routing key the Kotlin host already uses
/// (`OauthHandler.startOauth(provider, alias)`), so we don't need to
/// forward the plugin id here.
fn start(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let _pid = plugin_id(args)?;
    let provider = require_string(args, 1)?;
    let alias = {
        let v = args.get_or_undefined(2);
        if v.is_undefined() || v.is_null() {
            String::new()
        } else {
            v.as_string()
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default()
        }
    };

    #[cfg(target_os = "android")]
    {
        if let Err(e) = upcall_start_oauth(&provider, &alias) {
            tracing::warn!("ease:oauth.start({provider}) upcall failed: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (&provider, &alias);
        tracing::debug!("ease:oauth.start ignored (non-Android target)");
    }

    Ok(JsValue::undefined())
}

#[cfg(target_os = "android")]
fn upcall_start_oauth(provider: &str, alias: &str) -> Result<(), String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = super::host_cache::oauth_host_class().ok_or("oauth host class not cached")?;
    let class = unsafe { JClass::from_raw(raw_class) };
    let provider_jstr = env
        .new_string(provider)
        .map_err(|e| format!("new_string provider: {e}"))?;
    let alias_jstr = env
        .new_string(alias)
        .map_err(|e| format!("new_string alias: {e}"))?;
    env.call_static_method(
        class,
        "startOauth",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&provider_jstr), JValue::Object(&alias_jstr)],
    )
    .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(())
}
