//! `ease.oauth` JS bridge — OAuth flow-id minting + flow trigger for plugin
//! *setup views* (tur-rendered config forms).
//!
//! The host carries **identity only** — business data (e.g. the alias the
//! user typed) never crosses this bridge:
//!
//! 1. The plugin calls `ease.oauth.new()` → the host mints an opaque
//!    `oauthId` (an in-process counter token, unique for the process
//!    lifetime — exactly as long as the flow correlation needs to live).
//!    The plugin keys its own pending flow data by it in its own KV
//!    (e.g. `oauth:<oauthId>` = `{ alias, … }`), enabling concurrent flows.
//! 2. `ease.oauth.start(oauthId)` reads the calling plugin's identity from
//!    the per-instance data slot (no JS arg) and forwards
//!    `(pluginId, oauthId)` to the Kotlin host (`EaseOauthHost.startOauth`)
//!    via a JNI static-method upcall on the JVM the engine thread is
//!    already attached to. Kotlin then fetches the authorize URL via the
//!    headless service RPC (`oauth:url { pluginId, oauthId }`), stashes the
//!    pair, and opens the system browser; the `easem://oauth2redirect`
//!    callback in `MainActivity` completes the exchange
//!    (`oauth:exchange { pluginId, oauthId, code }`) — at which point the
//!    plugin's backend consumes its `oauth:<oauthId>` pending slot.
//!
//! `start` is a one-way trigger — it returns immediately; the redirect lands
//! asynchronously. Non-Android targets compile the upcall as a no-op (host
//! `cargo check` / UniFFI codegen only).
//!
//! The `JavaVM` is obtained from `ndk_context` (initialized once by
//! `EaseMusicPlayerApplication.nativeInitAndroidContext` for cpal's AAudio
//! backend) rather than re-cached here.

use std::sync::atomic::{AtomicU64, Ordering};

use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use crate::plugin_runtime::PluginId;

/// Monotonic source of opaque OAuth flow ids. Process-scoped by design: the
/// id only correlates a browser round-trip, and every host-side flow stash
/// dies with the process anyway.
static OAUTH_FLOW_SEQ: AtomicU64 = AtomicU64::new(1);

/// Build the `FnEntry` table for the `oauth` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![("new", 0, new as Ptr), ("start", 1, start as Ptr)]
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

/// `ease.oauth.new() -> oauthId` — mint a fresh, opaque OAuth flow id. The
/// host keeps no state for it; the plugin keys its pending flow data
/// (alias, …) by the id in its own KV and consumes it when the
/// `oauth:exchange` op arrives. The slot identity is checked only to gate
/// `ease.*` calls to plugin-bound instances — the minted id embeds nothing.
fn new(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    plugin_id(args)?;
    let n = OAUTH_FLOW_SEQ.fetch_add(1, Ordering::Relaxed);
    let token = format!("oauth-{n}");
    Ok(JsValue::from(js_string!(token.as_str())))
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

/// `ease.oauth.start(oauthId)` — fire the OAuth flow on the Kotlin host.
/// Returns immediately (the redirect completes asynchronously). The calling
/// plugin's identity comes from the per-instance data slot; the only JS
/// argument is the flow id from a matching `oauth.new()` call.
fn start(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let pid = plugin_id(args)?;
    let oauth_id = require_string(args, 1)?;

    #[cfg(target_os = "android")]
    {
        if let Err(e) = upcall_start_oauth(pid.as_ref(), &oauth_id) {
            tracing::warn!("ease:oauth.start({oauth_id}) upcall failed: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (pid.as_ref(), &oauth_id);
        tracing::debug!("ease:oauth.start ignored (non-Android target)");
    }

    Ok(JsValue::undefined())
}

#[cfg(target_os = "android")]
fn upcall_start_oauth(plugin_id: &str, oauth_id: &str) -> Result<(), String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = super::host_cache::oauth_host_class().ok_or("oauth host class not cached")?;
    let class = unsafe { JClass::from_raw(raw_class) };
    let plugin_jstr = env
        .new_string(plugin_id)
        .map_err(|e| format!("new_string plugin_id: {e}"))?;
    let oauth_jstr = env
        .new_string(oauth_id)
        .map_err(|e| format!("new_string oauth_id: {e}"))?;
    env.call_static_method(
        class,
        "startOauth",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&plugin_jstr), JValue::Object(&oauth_jstr)],
    )
    .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(())
}
