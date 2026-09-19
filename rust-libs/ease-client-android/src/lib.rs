//! ease-client-android — the Android embedder half of the ease client.
//!
//! Everything that needs a live JVM or an Android platform object lives in
//! this crate; the business crate ([`ease_client_backend`]) stays
//! platform-agnostic and host-testable (its `cargo nextest` suite runs on
//! macOS with no engine in sight). This crate is the cdylib the app loads
//! (`libease_client_android.so`, see `scripts/build-jni-libs.ts`) and hosts
//! **every** JNI symbol:
//!
//! - [`bridge_jni`] — `EaseBridge.call`, the JSON+buffer request/response
//!   surface all repositories ride on;
//! - [`plugin_runtime::plugin_jni`] — `TurNative.*` (engine ops) and
//!   `EasePluginBridge.*` (runtime + backend binding);
//! - this file — `EaseMusicPlayerApplication.nativeInitAndroidContext`
//!   (ndk-context registration, required by cpal's AAudio backend).
//!
//! The two halves meet through [`ease_client_backend::services::
//! plugin_manager::PluginEngineHost`]: `bindPluginRuntime` attaches a
//! [`plugin_runtime::engine::TurEngineHost`] (tur runtime handle + raw
//! `AAssetManager`) to the named backend instance, and the backend's plugin
//! scan / bootstrap call back through the trait. Host builds simply never
//! attach one — scans then report zero module-source handles plus loud
//! warnings instead of failing silently.

pub mod bridge_jni;
pub mod plugin_runtime;

use std::sync::OnceLock;

// ============================================================================
// JNI entrypoint for ndk-context initialization (required by cpal's AAudio
// backend on Android).
//
// cpal → ndk::audio → AAudio needs to query the JVM for the app's
// AudioManager (for hints like frames-per-buffer). That path goes through
// `ndk_context::android_context()`, which panics if nobody has registered
// the JavaVM + app Context. ndk-glue does this for native-activity apps,
// but we're a cdylib loaded by a normal Kotlin app, so we have to
// register the context ourselves at startup.
//
// Kotlin calls this once via `external fun` from
// EaseMusicPlayerApplication.onCreate:
//   companion object {
//     init { System.loadLibrary("ease_client_android") }
//     external fun nativeInitAndroidContext(context: android.content.Context)
//   }
// ============================================================================

static ANDROID_CONTEXT_CONFIGURED: OnceLock<()> = OnceLock::new();

#[allow(non_snake_case)]
#[no_mangle]
pub extern "system" fn Java_com_kutedev_easemusicplayer_EaseMusicPlayerApplication_nativeInitAndroidContext(
    env: jni::JNIEnv,
    _class: jni::objects::JClass,
    context: jni::objects::JObject,
) {
    // `configure_android_context` panics if called twice; guard with a OnceLock.
    if ANDROID_CONTEXT_CONFIGURED.get().is_some() {
        return;
    }
    let vm_ptr = env.get_java_vm();
    let vm = match vm_ptr {
        Ok(vm) => vm,
        Err(e) => {
            tracing::error!("nativeInitAndroidContext: get_java_vm failed: {e:?}");
            return;
        }
    };
    // Globalize the context reference so it outlives this JNI call.
    let global_context = env.new_global_ref(context);
    let global_context = match global_context {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("nativeInitAndroidContext: new_global_ref failed: {e:?}");
            return;
        }
    };
    // ndk-context wants raw void* pointers (JavaVM* + jobject).
    let raw_vm = vm.get_java_vm_pointer() as *mut std::ffi::c_void;
    let raw_ctx = global_context.as_raw() as *mut std::ffi::c_void;
    // SAFETY: raw_vm is a valid JavaVM* from jni-rs (process-lifetime); raw_ctx
    // is a global JNI ref we just created and will leak below. Both remain
    // valid for the process lifetime, which is what ndk-context requires.
    unsafe { ndk_context::initialize_android_context(raw_vm, raw_ctx) };
    // Leak the global ref — ndk-context now owns it for the process lifetime.
    std::mem::forget(global_context);
    let _ = ANDROID_CONTEXT_CONFIGURED.set(());
    tracing::info!("ndk_context configured for cpal AAudio backend");
}
