use std::sync::Arc;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
mod infra;
mod objects;
pub(crate) mod bridge;
pub(crate) mod plugin_runtime;
pub(crate) mod repositories;
mod services;
pub(crate) mod utils;

pub use objects::*;

pub use ease_remote_storage::StreamFile;
use error::BResult;

pub use crate::services::ArgInitializeApp;
use crate::{
    ctx::BackendContext,
    infra::init_infra,
    services::{app_bootstrap, app_destroy},
};

/// Process-wide handle to the active [`BackendContext`]. Set once by
/// [`Backend::init`] and read by the tur `EaseMusicPlugin` so its `ease:*`
/// JS bridge modules can call into the database / KV storage without a
/// direct dependency on the `Backend` object.
///
/// This is the in-memory integration seam between the ease backend and
/// the tur engine — both `.so` symbols live in the same process (tur is
/// linked as an rlib into `libease_client_backend.so`), so a OnceLock is
/// all that's needed for cross-module sharing.
pub(crate) static BACKEND_CONTEXT: std::sync::OnceLock<Arc<BackendContext>> = std::sync::OnceLock::new();

pub struct Backend {
    pub(crate) arg: ArgInitializeApp,
    cx: Arc<BackendContext>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

impl Backend {
    pub async fn init_async(&self) -> BResult<()> {
        let cx = self.cx.clone();
        let cx_for_once = self.cx.clone();
        let arg = self.arg.clone();
        app_bootstrap(&cx, arg).await?;
        // Publish the backend context for the tur EaseMusicPlugin. Set
        // before the first tur engine is constructed so ease:* modules
        // can resolve the context synchronously at register-time.
        let _ = BACKEND_CONTEXT.set(cx_for_once);
        Ok(())
    }

    /// Legacy sync entrypoint — must NOT be called from inside a tokio
    /// runtime context. Used by tests; the bridge dispatcher uses
    /// [`Backend::init_async`] instead.
    pub fn init(&self) -> BResult<()> {
        ease_client_tokio::tokio_runtime().block_on(self.init_async())
    }

    pub async fn deinit_async(&self) -> BResult<()> {
        app_destroy(&self.cx).await
    }

    pub fn deinit(&self) -> BResult<()> {
        ease_client_tokio::tokio_runtime().block_on(self.deinit_async())
    }
}

impl Backend {
    pub fn get_context(&self) -> &BackendContext {
        &self.cx
    }

    pub fn storage_path(&self) -> String {
        self.cx.get_storage_path()
    }
}

pub fn create_backend(arg: ArgInitializeApp) -> Arc<Backend> {
    let cx = Arc::new(BackendContext::new());
    init_infra(&arg.app_document_dir);
    Arc::new(Backend {
        cx,
        arg,
    })
}

pub fn ease_log(msg: &str) {
    tracing::info!("{}", msg);
}

pub fn ease_error(msg: &str) {
    tracing::error!("{}", msg);
}

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
// Kotlin calls this once via `external fun` from MainActivity.onCreate:
//   companion object {
//     init { System.loadLibrary("ease_client_backend") }
//     external fun nativeInitAndroidContext(context: android.content.Context)
//   }
// ============================================================================

static ANDROID_CONTEXT_CONFIGURED: std::sync::OnceLock<()> = std::sync::OnceLock::new();

#[allow(non_snake_case)]
#[no_mangle]
pub extern "system" fn Java_com_kutedev_easemusicplayer_EaseMusicPlayerApplication_nativeInitAndroidContext(
    mut env: jni::JNIEnv,
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
