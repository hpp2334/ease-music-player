//! JNI entry points for the tur engine, embedded inside
//! `libease_client_backend.so`.
//!
//! We hand-write **all** JNI entry points under the
//! `com.kutedev.easemusicplayer.turintegration` package (matching the
//! Kotlin classes `TurNative` / `EasePluginBridge`), instead of using
//! `tur_android::standard_jni_exports!()` (which emits symbols under
//! `org.tur.TurNative` and would require shipping Kotlin classes in
//! that package). Each wrapper is a one-line delegate to the matching
//! `tur_android::ops::*` function.
//!
//! All symbols are Android-only. On a non-Android host this file is
//! empty so the workspace still compiles for `cargo check`.

#![cfg(target_os = "android")]

use tur_engine::core::scheduler::WorkerPoolHandle;

/// The two capped worker pools all plugin instances share, registered on
/// the tur runtime at `createRuntime` and assigned per-app in
/// `createInstance` / `createHeadlessInstance` — instead of the engine
/// default (one dedicated lane thread per app):
///
/// - `backend` (cap 2): all headless service backends,
/// - `view` (cap 2): all TurView rendering instances.
///
/// Owned by the Kotlin host as an opaque `jlong` handle (boxed here), so
/// every entry point receives it explicitly — no process-global stash
/// whose handles could outlive a destroyed runtime.
struct PluginWorkerPools {
    backend: WorkerPoolHandle,
    view: WorkerPoolHandle,
}

impl PluginWorkerPools {
    fn new() -> Self {
        Self {
            backend: WorkerPoolHandle::new("ease-plugin-backend", 2),
            view: WorkerPoolHandle::new("ease-plugin-view", 2),
        }
    }
}

/// Borrow the pools from a Kotlin-held `PluginWorkerPools` handle. `0`
/// (Kotlin "no handle") yields `None` — callers then leave the engine's
/// default pool in place.
fn borrow_pools(pools: tur_android::jlong) -> Option<&'static PluginWorkerPools> {
    (pools != 0).then(|| unsafe { &*(pools as *const PluginWorkerPools) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_loadModule(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    source_handle: tur_android::jlong,
) {
    tur_android::ops::load_module(&mut env, handle, source_handle)
}

/// `TurNative.registerModuleSource(runtimeHandle, js): long` — register a
/// module source on the runtime's shared `ModuleSourceRegistry` and return
/// its opaque handle (`0` on failure). The source crosses JNI exactly once,
/// here; `loadModule` then loads it into any instance of the runtime by
/// handle — no per-load string copies.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_registerModuleSource(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    js: tur_android::JString,
) -> tur_android::jlong {
    tur_android::ops::register_module_source(&mut env, runtime_handle, js)
}

/// `TurNative.releaseModuleSource(runtimeHandle, sourceHandle)` — drop a
/// registered module source. Idempotent; a stale/unknown handle is a no-op.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_releaseModuleSource(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    source_handle: tur_android::jlong,
) {
    tur_android::ops::release_module_source(&mut env, runtime_handle, source_handle)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_pump(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) -> tur_android::jint {
    tur_android::ops::pump(handle)
}

/// `TurNative.pumpMessages(handle): int` — poll the engine's main loop
/// WITHOUT firing a vsync (the coalesced message-pump path; keeps an idle
/// instance at 0% CPU instead of ping-ponging at display refresh rate).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_pumpMessages(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) -> tur_android::jint {
    tur_android::ops::pump_messages(handle)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_resize(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    width: tur_android::jint,
    height: tur_android::jint,
    dpr: tur_android::jdouble,
) {
    tur_android::ops::resize(&mut env, handle, width, height, dpr)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_pushPointer(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    action: tur_android::jint,
    x: tur_android::jdouble,
    y: tur_android::jdouble,
    time_ms: tur_android::jlong,
) {
    tur_android::ops::push_pointer(&mut env, handle, action, x, y, time_ms)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_pushKey(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    key: tur_android::JString,
    code: tur_android::JString,
    action: tur_android::jint,
    ctrl: tur_android::jboolean,
    shift: tur_android::jboolean,
    alt: tur_android::jboolean,
    meta: tur_android::jboolean,
) {
    tur_android::ops::push_key(&mut env, handle, key, code, action, ctrl, shift, alt, meta)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_pushIme(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    kind: tur_android::jint,
    text: tur_android::JString,
) {
    tur_android::ops::push_ime(&mut env, handle, kind, text)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_destroy(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) {
    tur_android::ops::destroy(handle)
}

/// `TurNative.destroySettled(handle): boolean` — destroy plus a fence:
/// blocks until the tur-host op queue drained past this instance's
/// destroy op (FIFO), i.e. until the instance, its renderer + surface,
/// and its loop future are dropped. The fence for disposal-sensitive
/// teardown (replacing sleep-based quiesce heuristics); **off-main-thread
/// only** — it can wait behind an in-flight build. Returns `true` when
/// settled, `false` if the host thread had shut down.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_destroySettled(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) -> tur_android::jboolean {
    tur_android::ops::destroy_settled(handle)
}

/// `TurNative.createInstance(runtimeHandle, poolsHandle, frameLoop, pluginId, instance): long`
///
/// Spawns an isolated **renderer-less** instance for the given `pluginId`
/// — the INITIALIZE half of tur's two-phase lifecycle (#215). The plugin
/// id is stamped into the instance's per-instance data slot at build time
/// (via `TurAppBuilder::instance_data`) so `ease:*` bridge fns can resolve
/// the calling plugin via `extract_js_ctx` + `data::<PluginId>()` —
/// without trusting a JS argument. No surface is involved: the Kotlin
/// host ATTACHES one later via [`Java_..._TurNative_attachInstance`]
/// (`surfaceCreated`), and an instance that never attaches is simply
/// headless.
///
/// `poolsHandle` assigns the instance to the shared `ease-plugin-view`
/// worker pool (see [`PluginWorkerPools`]); `0` keeps the engine default.
///
/// `instance` is the storage's `plugin_storage_id` for edit-mode views
/// (stamped as [`PluginInstance::Some`]); pass an empty string for
/// create-mode setup views (stamped as [`PluginInstance::None`]).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_createInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    pools_handle: tur_android::jlong,
    frame_loop: tur_android::JObject,
    plugin_id: tur_android::JString,
    instance: tur_android::JString,
) -> tur_android::jlong {
    let pid: String = match env.get_string(&plugin_id) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("createInstance: get_string(plugin_id) failed: {e}");
            return 0;
        }
    };
    let instance_str: String = match env.get_string(&instance) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("createInstance: get_string(instance) failed: {e}");
            return 0;
        }
    };
    let instance_opt: Option<String> = if instance_str.is_empty() {
        None
    } else {
        Some(instance_str)
    };
    let view_pool = borrow_pools(pools_handle).map(|pools| pools.view.clone());
    tur_android::ops::create_instance(&mut env, runtime_handle, frame_loop, move |builder| {
        let builder = match view_pool {
            Some(ref pool) => builder.worker_pool(pool.clone()),
            None => builder,
        };
        builder.instance_data(move |cx| {
            cx.define::<crate::plugin_runtime::PluginId>(crate::plugin_runtime::PluginId::new(
                pid.clone(),
            ));
            cx.define::<crate::plugin_runtime::PluginInstance>(
                crate::plugin_runtime::PluginInstance(instance_opt.clone()),
            );
        })
    })
}

/// `TurNative.createHeadlessInstance(runtimeHandle, poolsHandle, frameLoop, pluginId): long`
///
/// Headless variant — same per-instance `PluginId` stamping as
/// [`Java_..._TurNative_createInstance`], never attached to a surface.
/// Since tur #215 the engine has a single renderer-less
/// `create_instance` op (a never-attached instance IS headless), so this
/// only differs in the worker pool: `poolsHandle` assigns the instance to
/// the shared `ease-plugin-backend` pool (see [`PluginWorkerPools`]); `0`
/// keeps the engine default.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_createHeadlessInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    pools_handle: tur_android::jlong,
    frame_loop: tur_android::JObject,
    plugin_id: tur_android::JString,
) -> tur_android::jlong {
    let pid: String = match env.get_string(&plugin_id) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("createHeadlessInstance: get_string(plugin_id) failed: {e}");
            return 0;
        }
    };
    let backend_pool = borrow_pools(pools_handle).map(|pools| pools.backend.clone());
    tur_android::ops::create_instance(&mut env, runtime_handle, frame_loop, move |builder| {
        let builder = match backend_pool {
            Some(ref pool) => builder.worker_pool(pool.clone()),
            None => builder,
        };
        builder.instance_data(move |cx| {
            cx.define::<crate::plugin_runtime::PluginId>(crate::plugin_runtime::PluginId::new(
                pid.clone(),
            ));
            cx.define::<crate::plugin_runtime::PluginInstance>(
                crate::plugin_runtime::PluginInstance(None),
            );
        })
    })
}

/// `TurNative.attachInstance(handle, surface, width, height, dpr)` — the
/// ATTACH half of tur's two-phase lifecycle (#215). Call from
/// `surfaceCreated`, where the `Surface` is guaranteed valid. The attach
/// op (FIFO-ordered behind the instance build, so the instance exists
/// when it runs) acquires the `ANativeWindow`, performs the wgpu
/// surface/adapter/device init, and hands the renderer to the engine. On
/// failure the instance stays renderer-less and attachable again. Pair
/// with [`Java_..._TurNative_detachInstance`]; the pair is repeatable —
/// a re-created surface re-attaches without rebuilding the JS realm.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_attachInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    surface: tur_android::JObject,
    width: tur_android::jint,
    height: tur_android::jint,
    dpr: tur_android::jdouble,
) {
    tur_android::ops::attach_instance(&mut env, handle, surface, width, height, dpr)
}

/// `TurNative.detachInstance(handle)` — the DETACH half (#215). Call from
/// `surfaceDestroyed`. Drops the renderer (releasing the native window
/// ref) while the instance keeps running (JS, capabilities, events) and
/// can attach a fresh surface later. Idempotent.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_detachInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) {
    tur_android::ops::detach_instance(&mut env, handle)
}

/// `TurNative.destroyRuntime(handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_destroyRuntime(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) {
    tur_android::ops::destroy_runtime(handle)
}

/// `EasePluginBridge.createPluginWorkerPools(): long` — allocate the two
/// capped shared worker pools (see [`PluginWorkerPools`]) and return an
/// opaque handle for the Kotlin host to pass back into `createRuntime` /
/// `createInstance` / `createHeadlessInstance`. Returns `0` on failure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createPluginWorkerPools(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
) -> tur_android::jlong {
    Box::into_raw(Box::new(PluginWorkerPools::new())) as tur_android::jlong
}

/// `EasePluginBridge.destroyPluginWorkerPools(poolsHandle)` — free pools
/// allocated by [`Java_..._EasePluginBridge_createPluginWorkerPools`].
/// Call after `destroyRuntime`. `0` is a no-op. Idempotent per handle.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_destroyPluginWorkerPools(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    pools_handle: tur_android::jlong,
) {
    if pools_handle != 0 {
        drop(unsafe { Box::from_raw(pools_handle as *mut PluginWorkerPools) });
    }
}

/// `EasePluginBridge.createRuntime(env, context, poolsHandle): long` — builds the shared
/// tur runtime once, with the Ease plugin set registered on it. Instances
/// (one per TurView, or a headless one for a service plugin) are spawned from
/// it via `TurNative.createInstance` / `createHeadlessInstance`. A non-zero
/// `poolsHandle` also registers the shared plugin worker pools on the
/// runtime; `0` falls back to the engine default (one lane per instance).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
    pools_handle: tur_android::jlong,
) -> tur_android::jlong {
    use tur_animation::TurAnimationPlugin;
    use tur_engine::{TurClipboardPlugin, TurStdPlugin};
    use tur_net_native::{Http, NativeHttp, TurNetPlugin};

    use crate::plugin_runtime::EaseMusicPlugin;
    use ease_tur_rpc::TurRpcPlugin;

    // Cache global refs to the Kotlin host classes (EaseOauthHost /
    // EaseThemesHost) so the tur worker thread can call static methods on
    // them — the worker attaches with the system ClassLoader and can't
    // find_class app classes. Must run here (main thread, app ClassLoader).
    if let Err(e) = crate::plugin_runtime::host_cache::cache_host_classes(&mut env) {
        tracing::warn!("host_cache: {e}");
    }

    // Clone up front — the builder closure must be 'static and the Kotlin
    // host keeps the pools handle alive independently of this runtime.
    let pools = borrow_pools(pools_handle).map(|pools| (pools.backend.clone(), pools.view.clone()));
    if pools_handle != 0 && pools.is_none() {
        tracing::error!("createRuntime: nonzero poolsHandle is not a live PluginWorkerPools");
    }

    tur_android::ops::create_runtime(&mut env, context, move |builder| {
        // tur's engine core is tokio-free (since the drop-tokio refactor); the
        // embedder must hand NativeHttp a Handle onto a runtime it owns + keeps
        // alive for the engine's lifetime. We use the shared ease-client-tokio
        // runtime (same one the backend + JsStorageBackend spawn onto). The
        // builder's capability() takes a closure that may receive the engine's
        // AsyncPluginContext; NativeHttp needs only the tokio Handle.
        //
        // `move`: the closure is marshalled onto the tur-host thread
        // (#210), so it must be 'static — `pools` was already cloned up
        // front for exactly this.
        let handle = ease_client_tokio::tokio_runtime().handle().clone();
        let builder = match pools {
            Some((ref backend, ref view)) => builder
                .worker_pool(backend.clone())
                .worker_pool(view.clone()),
            None => builder,
        };
        builder
            .capability(move |_| Ok(Http::new(NativeHttp::new(handle.clone()))))
            .plugin(TurStdPlugin)
            .plugin(TurAnimationPlugin)
            .plugin(TurClipboardPlugin)
            .plugin(TurNetPlugin)
            .plugin(TurRpcPlugin)
            .plugin(EaseMusicPlugin)
    })
}

/// `EasePluginBridge.wireServiceRpc(instanceHandle, pluginId): boolean` —
/// connects the headless service instance's event bus to ease-tur-rpc and
/// stashes the resulting `Send` [`RpcClient`] into the global backend
/// context under `pluginId`. Called once per plugin, on the instance's own
/// thread (the JNI thread) after `createHeadlessInstance` +
/// `loadModule(backend.js)`. Returns `true` on success.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_wireServiceRpc(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    instance_handle: tur_android::jlong,
    plugin_id: tur_android::JString,
) -> tur_android::jboolean {
    let pid: String = match env.get_string(&plugin_id) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("wireServiceRpc: get_string(plugin_id) failed: {e}");
            return 0;
        }
    };
    let Some(rpc) =
        tur_android::ops::with_app(instance_handle, |app| ease_tur_rpc::RpcClient::wire(app))
    else {
        tracing::error!("wireServiceRpc: invalid instance handle");
        return 0;
    };
    match rpc {
        Ok(client) => {
            if let Some(cx) = crate::BACKEND_CONTEXT.get() {
                cx.set_service_rpc(&pid, client);
                tracing::info!("wireServiceRpc: service RpcClient installed for {pid}");
                1
            } else {
                tracing::error!("wireServiceRpc: BACKEND_CONTEXT not set");
                0
            }
        }
        Err(e) => {
            tracing::error!("wireServiceRpc: RpcClient::wire failed: {e}");
            0
        }
    }
}

/// `EasePluginBridge.unwireServiceRpc(pluginId)` — drop the backend
/// context's service `RpcClient` entry for `plugin_id` (its headless
/// instance is being torn down: the plugin was disabled / uninstalled /
/// upgraded). Storage dispatch + event delivery for the plugin degrade
/// gracefully (miss on `service_rpc_for`) until a fresh instance is wired.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_unwireServiceRpc(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    plugin_id: tur_android::JString,
) {
    let pid: String = match env.get_string(&plugin_id) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("unwireServiceRpc: get_string(plugin_id) failed: {e}");
            return;
        }
    };
    if let Some(cx) = crate::BACKEND_CONTEXT.get() {
        cx.remove_service_rpc(&pid);
        tracing::info!("unwireServiceRpc: service RpcClient removed for {pid}");
    } else {
        tracing::error!("unwireServiceRpc: BACKEND_CONTEXT not set");
    }
}

// ============================================================================
// Minimal NDK asset FFI (libandroid.so) — reading the bundled plugin zip
// natively so its bytes never cross the JNI boundary. Pattern lifted from
// tur's compose demo (`createAssetModuleSource`).
// ============================================================================

#[repr(C)]
struct AAssetManager {
    _unused: [u8; 0],
}

#[repr(C)]
struct AAsset {
    _unused: [u8; 0],
}

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
/// [`Java_..._EasePluginBridge_bindPluginRuntime`]. Thread-safe
/// (`AAssetManager_open` is); called from the bridge dispatcher's IO
/// thread during `plugin.bootstrap`.
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

/// `EasePluginBridge.bindPluginRuntime(runtimeHandle, assetManager)` — hand
/// the (already-created) tur runtime handle to the backend so
/// `plugin.list` can register module sources on it (tur #198), and stash
/// the raw `AAssetManager` pointer for reading bundled plugin zips during
/// `plugin.bootstrap`. The AssetManager object is owned by the application
/// Context for the app lifetime, so the raw pointer stays valid. Call once
/// after `EasePluginBridge.runtime(context)` (which does it automatically).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_bindPluginRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    asset_manager: tur_android::JObject,
) {
    let Some(cx) = crate::BACKEND_CONTEXT.get() else {
        tracing::error!("bindPluginRuntime: BACKEND_CONTEXT not set (bridge.initialize first)");
        return;
    };
    let mgr = unsafe {
        AAssetManager_fromJava(
            env.get_raw() as *mut std::ffi::c_void,
            asset_manager.as_raw() as *mut std::ffi::c_void,
        )
    };
    if mgr.is_null() {
        tracing::error!("bindPluginRuntime: AAssetManager_fromJava returned null");
        return;
    }
    let shared = cx.plugin_manager();
    shared.set_runtime_handle(runtime_handle);
    shared.set_asset_manager(mgr as usize);
    tracing::info!("bindPluginRuntime: runtime handle {runtime_handle} + asset manager bound");
}
