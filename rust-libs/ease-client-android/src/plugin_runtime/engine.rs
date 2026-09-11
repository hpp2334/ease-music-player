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

use std::collections::HashMap;
use std::sync::Arc;

use ease_client_backend::services::plugin_manager::PluginEngineHost;

/// The host implementation. Constructed by the `bindPluginRuntime` JNI
/// trampoline ([`crate::plugin_runtime::plugin_jni`]) and attached to the
/// backend context's plugin-manager shared state; dropped (compare-and-set)
/// by `unbindPluginRuntime`.
///
/// Headless-instance spawn/load/close run on whatever thread the backend's
/// reload lands on (a tokio worker): the JVM-dependent steps attach that
/// thread to the JVM via ndk-context for the duration of the call.
pub(crate) struct TurEngineHost {
    runtime_handle: i64,
    pools_handle: i64,
    asset_manager: usize,
    /// Per spawned headless instance: the Kotlin `FrameLoop` global ref +
    /// its `AtomicLong` handle cell (the zeroable pump guard — see
    /// `FrameLoop.closeInstance`). Dropped when the instance closes.
    loops: std::sync::Mutex<HashMap<i64, (jni::objects::GlobalRef, jni::objects::GlobalRef)>>,
}

impl TurEngineHost {
    pub(crate) fn new(runtime_handle: i64, pools_handle: i64, asset_manager: usize) -> Self {
        Self {
            runtime_handle,
            pools_handle,
            asset_manager,
            loops: Default::default(),
        }
    }
}

/// Attach the current (native) thread to the JVM via ndk-context for the
/// duration of `f`. Works from any thread — the tokio workers the backend
/// reload runs on, or an already-attached JNI thread.
#[cfg(target_os = "android")]
fn with_attached_env<R>(
    f: impl FnOnce(&mut jni::JNIEnv) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let ctx = ndk_context::android_context();
    // SAFETY: the JavaVM pointer registered by `nativeInitAndroidContext`
    // is valid for the process lifetime.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut _) }?;
    let mut env = vm.attach_current_thread().map_err(|e| anyhow::anyhow!("{e}"))?;
    f(&mut env)
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

    /// Spawn a headless instance from Rust: attach to the JVM, construct
    /// the Kotlin `FrameLoop` (main-looper-safe by construction), hand it
    /// to the engine's `create_instance` (which globalizes the ref itself),
    /// then wire the loop's wake callbacks through a zeroable
    /// `AtomicLong` cell — the exact `TurInstance` guard semantics, so a
    /// wake that lands after `close_headless` reads 0 and no-ops instead of
    /// pumping a freed route.
    fn spawn_headless(&self, plugin_id: &str) -> anyhow::Result<i64> {
        #[cfg(target_os = "android")]
        {
            use tur_engine::core::render::brush::Color;

            let pid = plugin_id.to_string();
            let pools_handle = self.pools_handle;
            let runtime_handle = self.runtime_handle;
            let loops = &self.loops;
            with_attached_env(move |env| {
                let backend_pool =
                    crate::plugin_runtime::plugin_jni::borrow_pools(pools_handle)
                        .map(|pools| pools.backend.clone());
                let class = crate::plugin_runtime::host_cache::frame_loop_class()
                    .ok_or_else(|| anyhow::anyhow!("FrameLoop class not cached (createRuntime not run)"))?;
                // SAFETY: the cached raw jclass is a process-lifetime global ref.
                let class = unsafe { jni::objects::JClass::from_raw(class) };
                let loop_obj: jni::objects::JObject =
                    env.new_object(&class, "()V", &[]).map_err(|e| anyhow::anyhow!("FrameLoop ctor: {e}"))?;
                // Global refs up front: `create_instance` takes the JObject
                // by value (and globalizes it for the engine's FrameLoopRef);
                // our own references for the wiring + close statics are these.
                let loop_global = env
                    .new_global_ref(&loop_obj)
                    .map_err(|e| anyhow::anyhow!("global ref (loop): {e}"))?;
                let instance = tur_android::ops::create_instance(
                    env,
                    runtime_handle,
                    loop_obj,
                    move |builder| {
                        let builder = match backend_pool {
                            Some(ref pool) => builder.worker_pool(pool.clone()),
                            None => builder,
                        };
                        builder.instance_data(move |cx| {
                            cx.define::<crate::plugin_runtime::PluginId>(
                                crate::plugin_runtime::PluginId::new(pid.clone()),
                            );
                            cx.define::<crate::plugin_runtime::PluginInstance>(
                                crate::plugin_runtime::PluginInstance(None),
                            );
                        })
                    },
                    // Never attached to a surface — the base color is moot.
                    Color::WHITE,
                );
                if instance == 0 {
                    anyhow::bail!("create_instance returned 0 (see logcat)");
                }
                // Guard cell + wake wiring (mirrors TurInstance.init).
                let cell = env
                    .new_object(
                        "java/util/concurrent/atomic/AtomicLong",
                        "(J)V",
                        &[jni::objects::JValue::Long(instance)],
                    )
                    .map_err(|e| anyhow::anyhow!("AtomicLong ctor: {e}"))?;
                let cell_global = env
                    .new_global_ref(&cell)
                    .map_err(|e| anyhow::anyhow!("global ref (cell): {e}"))?;
                env.call_static_method(
                    &class,
                    "wireToInstance",
                    "(Lcom/kutedev/easemusicplayer/turintegration/FrameLoop;Ljava/util/concurrent/atomic/AtomicLong;)V",
                    &[(&*loop_global).into(), (&cell).into()],
                )
                .map_err(|e| anyhow::anyhow!("FrameLoop.wireToInstance: {e}"))?;
                loops
                    .lock()
                    .unwrap()
                    .insert(instance, (loop_global, cell_global));
                Ok(instance)
            })
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = plugin_id;
            anyhow::bail!("headless spawn requires Android")
        }
    }

    fn load_headless_module(&self, instance_handle: i64, source_handle: i64) {
        #[cfg(target_os = "android")]
        {
            // `ops::load_module` takes an env it never uses for anything but
            // exception handling — attach for the call to preserve the exact
            // engine path (posted, FIFO behind the spawn).
            let _ = with_attached_env(|env| {
                tur_android::ops::load_module(env, instance_handle, source_handle);
                Ok(())
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (instance_handle, source_handle);
        }
    }

    fn extract_rpc(&self, instance_handle: i64) -> Option<ease_tur_rpc::RpcClient> {
        #[cfg(target_os = "android")]
        {
            // Blocking round-trip onto the engine host thread — FIFO behind
            // the spawn + load posts, so the JS handlers are registered by
            // the time this runs.
            tur_android::ops::with_app(instance_handle, |app| {
                ease_tur_rpc::RpcClient::wire(app)
            })
            .and_then(|r| match r {
                Ok(client) => Some(client),
                Err(e) => {
                    tracing::error!("RpcClient::wire failed: {e}");
                    None
                }
            })
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = instance_handle;
            None
        }
    }

    fn close_headless(&self, instance_handle: i64) {
        #[cfg(target_os = "android")]
        {
            let globals = self.loops.lock().unwrap().remove(&instance_handle);
            let _ = with_attached_env(|env| {
                if let Some((loop_ref, cell_ref)) = globals {
                    let class = crate::plugin_runtime::host_cache::frame_loop_class()
                        .ok_or_else(|| anyhow::anyhow!("FrameLoop class not cached"))?;
                    // SAFETY: process-lifetime global ref.
                    let class = unsafe { jni::objects::JClass::from_raw(class) };
                    env.call_static_method(
                        &class,
                        "closeInstance",
                        "(Lcom/kutedev/easemusicplayer/turintegration/FrameLoop;Ljava/util/concurrent/atomic/AtomicLong;)V",
                        &[(&*loop_ref).into(), (&*cell_ref).into()],
                    )
                    .map_err(|e| anyhow::anyhow!("FrameLoop.closeInstance: {e}"))?;
                }
                Ok(())
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = instance_handle;
        }
    }

    fn emit_signal(&self, op: u32, payload: &str) {
        #[cfg(target_os = "android")]
        {
            let payload = payload.to_string();
            let _ = with_attached_env(|env| {
                let class = crate::plugin_runtime::host_cache::signal_host_class()
                    .ok_or_else(|| anyhow::anyhow!("EaseSignalHost class not cached"))?;
                // SAFETY: process-lifetime global ref.
                let class = unsafe { jni::objects::JClass::from_raw(class) };
                let jstr = env
                    .new_string(&payload)
                    .map_err(|e| anyhow::anyhow!("new_string: {e}"))?;
                env.call_static_method(
                    &class,
                    "onSignal",
                    "(ILjava/lang/String;)V",
                    &[jni::objects::JValue::Int(op as i32), (&jstr).into()],
                )
                .map_err(|e| anyhow::anyhow!("EaseSignalHost.onSignal: {e}"))?;
                Ok(())
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (op, payload);
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
/// context's plugin-manager shared state and kick the first backend reload.
pub(crate) fn attach(cx: &ease_client_backend::ctx::BackendContext, host: TurEngineHost) {
    cx.plugin_manager().attach_engine_host(Arc::new(host));
    ease_client_backend::services::plugin_manager::spawn_reload_backends(cx);
}
