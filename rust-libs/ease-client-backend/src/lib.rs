use std::sync::Arc;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
mod infra;
mod objects;
pub(crate) mod repositories;
mod services;
mod streaming_server;
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

uniffi::setup_scaffolding!();

#[derive(uniffi::Object)]
pub struct Backend {
    arg: ArgInitializeApp,
    cx: Arc<BackendContext>,
    streaming_server: std::sync::Mutex<Option<streaming_server::StreamingServer>>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

#[uniffi::export]
impl Backend {
    pub fn init(&self) -> BResult<()> {
        let cx = self.cx.clone();
        let arg = self.arg.clone();
        ease_client_tokio::tokio_runtime().block_on(async move {
            app_bootstrap(&cx, arg).await
        })?;

        let server = streaming_server::StreamingServer::start(self.cx.weak());
        tracing::info!("streaming server started at {}", server.base_url());
        *self.streaming_server.lock().unwrap() = Some(server);
        Ok(())
    }

    pub fn deinit(&self) -> BResult<()> {
        *self.streaming_server.lock().unwrap() = None;
        ease_client_tokio::tokio_runtime().block_on(async {
            app_destroy(&self.cx).await
        })
    }

    /// Returns the streaming HTTP server's base URL (e.g.
    /// `http://127.0.0.1:54321`), or `None` if `init()` has not been
    /// called yet. JavaFX MediaPlayer points at `<base_url>/music/:id`.
    pub fn streaming_base_url(&self) -> Option<String> {
        self.streaming_server
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.base_url().to_string())
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

#[uniffi::export]
pub fn create_backend(arg: ArgInitializeApp) -> Arc<Backend> {
    let cx = Arc::new(BackendContext::new());
    init_infra(&arg.app_document_dir);
    Arc::new(Backend {
        cx,
        arg,
        streaming_server: std::sync::Mutex::new(None),
    })
}

#[uniffi::export]
pub fn ease_log(msg: &str) {
    tracing::info!("{}", msg);
}

#[uniffi::export]
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
// but we're a UniFFI cdylib loaded by a normal Kotlin app, so we have to
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
