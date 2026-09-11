//! The Android implementation of the backend's [`PluginEngineHost`] seam.
//!
//! One [`TurEngineHost`] exists per `bindPluginRuntime` call: it bundles the
//! opaque tur runtime handle (the binding's identity — what
//! `detach_engine_host` compare-and-sets against) with the raw
//! `*mut AAssetManager` stashed from the Kotlin `AssetManager` object. The
//! backend's plugin scan calls [`PluginEngineHost::register_source`] to put
//! module JS on the runtime's shared registry, and `plugin.bootstrap` calls
//! [`PluginEngineHost::read_bundled_asset`] to read the bundled plugin zips
//! natively — their bytes never cross the JNI boundary.

use std::sync::Arc;

use ease_client_backend::services::plugin_manager::PluginEngineHost;

/// The host implementation. Constructed by the `bindPluginRuntime` JNI
/// trampoline ([`crate::plugin_runtime::plugin_jni`]) and attached to the
/// backend context's plugin-manager shared state; dropped (compare-and-set)
/// by `unbindPluginRuntime`.
pub(crate) struct TurEngineHost {
    runtime_handle: i64,
    asset_manager: usize,
}

impl TurEngineHost {
    pub(crate) fn new(runtime_handle: i64, asset_manager: usize) -> Self {
        Self {
            runtime_handle,
            asset_manager,
        }
    }
}

impl PluginEngineHost for TurEngineHost {
    fn id(&self) -> i64 {
        self.runtime_handle
    }

    /// Register module JS on the bound runtime's shared
    /// `ModuleSourceRegistry` (tur #198) and return its opaque handle.
    /// `0` on failure — loudly, never silently: a zero handle would leave a
    /// plugin view/backend permanently blank.
    fn register_source(&self, src: String) -> i64 {
        if self.runtime_handle == 0 {
            tracing::error!(
                "register_module_source: no tur runtime bound — bindPluginRuntime not called \
                 / already unbound; returning 0"
            );
            return 0;
        }
        #[cfg(target_os = "android")]
        {
            match tur_android::ops::with_runtime(self.runtime_handle, |rt| {
                rt.module_sources.register(src) as i64
            }) {
                Some(handle) => handle,
                None => {
                    tracing::error!(
                        "register_module_source: runtime handle {} not found — stale binding? \
                         returning 0",
                        self.runtime_handle
                    );
                    0
                }
            }
        }
        // Non-Android host build (the android crate still type-checks there):
        // there is never a live runtime to register against.
        #[cfg(not(target_os = "android"))]
        {
            let _ = src;
            0
        }
    }

    fn read_bundled_asset(&self, path: &str) -> Option<Vec<u8>> {
        #[cfg(target_os = "android")]
        {
            read_asset_bytes(self.asset_manager, path)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = path;
            None
        }
    }
}

// ============================================================================
// Minimal NDK asset FFI (libandroid.so) — reading the bundled plugin zip
// natively so its bytes never cross the JNI boundary. Pattern lifted from
// tur's compose demo (`createAssetModuleSource`).
// ============================================================================

#[repr(C)]
pub struct AAssetManager {
    _unused: [u8; 0],
}

#[repr(C)]
struct AAsset {
    _unused: [u8; 0],
}

#[cfg(target_os = "android")]
#[link(name = "android")]
unsafe extern "C" {
    fn AAssetManager_fromJava(
        env: *mut std::ffi::c_void,
        asset_manager: *mut std::ffi::c_void,
    ) -> *mut AAssetManager;
    fn AAssetManager_open(
        mgr: *mut AAssetManager,
        filename: *const std::ffi::c_char,
        mode: i32,
    ) -> *mut AAsset;
    fn AAsset_getLength(asset: *mut AAsset) -> u64;
    fn AAsset_read(asset: *mut AAsset, buf: *mut std::ffi::c_void, count: usize) -> i32;
    fn AAsset_close(asset: *mut AAsset);
}

/// Read an APK asset fully, given the raw `*mut AAssetManager` stashed by
/// [`crate::plugin_runtime::plugin_jni`]'s `bindPluginRuntime`. Thread-safe
/// (`AAssetManager_open` is); called from the bridge dispatcher's IO
/// thread during `plugin.bootstrap`.
#[cfg(target_os = "android")]
pub(crate) fn read_asset_bytes(mgr: usize, path: &str) -> Option<Vec<u8>> {
    let mgr = mgr as *mut AAssetManager;
    if mgr.is_null() {
        return None;
    }
    let c_path = std::ffi::CString::new(path).ok()?;
    // 3 == AASSET_MODE_BUFFER: read the whole asset up front.
    let asset = unsafe { AAssetManager_open(mgr, c_path.as_ptr(), 3) };
    if asset.is_null() {
        return None;
    }
    let len = unsafe { AAsset_getLength(asset) } as usize;
    let mut buf = vec![0u8; len];
    // `AAsset_read` may return short reads — loop until full or EOF.
    let mut filled = 0usize;
    while filled < len {
        let n = unsafe {
            AAsset_read(
                asset,
                buf[filled..].as_mut_ptr() as *mut std::ffi::c_void,
                len - filled,
            )
        };
        if n <= 0 {
            break;
        }
        filled += n as usize;
    }
    unsafe { AAsset_close(asset) };
    buf.truncate(filled);
    Some(buf)
}

/// `AAssetManager_fromJava` — needs a live `JNIEnv` (the raw pointer), so
/// it is called from the `bindPluginRuntime` JNI trampoline, which has one.
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub(crate) unsafe fn aasset_manager_from_java(
    env: *mut std::ffi::c_void,
    asset_manager: *mut std::ffi::c_void,
) -> *mut AAssetManager {
    unsafe { AAssetManager_fromJava(env, asset_manager) }
}

/// Convenience for the JNI trampolines: attach the host impl to a backend
/// context's plugin-manager shared state.
pub(crate) fn attach(cx: &ease_client_backend::ctx::BackendContext, host: TurEngineHost) {
    cx.plugin_manager()
        .attach_engine_host(Arc::new(host));
}
