//! JNI entry points for the tur engine, embedded inside
//! `libease_client_android.so` (the `ease-client-android` cdylib).
//!
//! We hand-write **all** JNI entry points under the
//! `com.kutedev.easemusicplayer.turintegration` package (matching the
//! Kotlin classes `TurNative` / `EasePluginBridge`), instead of using
//! `tur_android::standard_jni_exports!()` (which emits symbols under
//! `org.tur.TurNative`). Each wrapper is a one-line delegate to the
//! matching `tur_android::ops::*` function.
//!
//! Worker pools live Rust-side now: `createRuntime` allocates the two
//! capped shared pools and files them under the runtime handle in a
//! process-static map; `createInstance` (views) and the backend crate's
//! `spawn_headless` (headless) pick their pool from it. There is no
//! Kotlin-held pools handle anymore.
//!
//! All symbols are Android-only. On a non-Android host this file is
//! empty so the workspace still compiles for `cargo check`.

#![cfg(target_os = "android")]

use std::collections::HashMap;
use std::sync::Mutex;

use tur_engine::core::render::brush::Color;
use tur_engine::core::scheduler::WorkerPoolHandle;

/// The two capped worker pools all plugin instances share, registered on
/// the tur runtime at `createRuntime` and assigned per-instance:
///
/// - `backend` (cap 2): all headless service backends,
/// - `view` (cap 2): all TurView rendering instances.
#[derive(Clone, Copy)]
pub(crate) struct PluginWorkerPools {
    pub(crate) backend: WorkerPoolHandle,
    pub(crate) view: WorkerPoolHandle,
}

impl PluginWorkerPools {
    fn new() -> Self {
        Self {
            backend: WorkerPoolHandle::new("ease-plugin-backend", 2),
            view: WorkerPoolHandle::new("ease-plugin-view", 2),
        }
    }
}

/// The pools per live runtime handle, filed by `createRuntime` and removed
/// by `destroyRuntime`. The engine's headless spawns and the view
/// `createInstance` both resolve their pool here.
static POOLS: Mutex<Option<HashMap<i64, PluginWorkerPools>>> = Mutex::new(None);

fn file_pools(runtime_handle: i64, pools: PluginWorkerPools) {
    POOLS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(runtime_handle, pools);
}

fn drop_pools(runtime_handle: i64) {
    if let Some(map) = POOLS.lock().unwrap().as_mut() {
        map.remove(&runtime_handle);
    }
}

/// The backend worker pool registered for `runtime_handle` (`None` = leave
/// the engine default).
pub(crate) fn backend_pool_for(runtime_handle: i64) -> Option<WorkerPoolHandle> {
    POOLS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&runtime_handle))
        .map(|p| p.backend.clone())
}

/// The view worker pool registered for `runtime_handle`.
fn view_pool_for(runtime_handle: i64) -> Option<WorkerPoolHandle> {
    POOLS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&runtime_handle))
        .map(|p| p.view.clone())
}

// ---------------------------------------------------------------------------
// TurNative one-liners (delegate to tur_android::ops)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_loadModule(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    source_handle: tur_android::jlong,
) {
    tur_android::ops::load_module(&mut env, handle, source_handle)
}

/// `TurNative.registerModuleSource(runtimeHandle, source): long` — register
/// a module rut source on the runtime's shared `ModuleSourceRegistry` and
/// return its opaque handle (`0` on failure). The source crosses JNI
/// exactly once, here; `loadModule` then loads it into any instance of the
/// runtime by handle — no per-load string copies.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_registerModuleSource(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    source: tur_android::JString,
) -> tur_android::jlong {
    tur_android::ops::register_module_source(&mut env, runtime_handle, source)
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
/// teardown; **off-main-thread only** — it can wait behind an in-flight
/// build. Returns `true` when settled, `false` if the host thread had shut
/// down.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_destroySettled(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) -> tur_android::jboolean {
    tur_android::ops::destroy_settled(handle)
}

/// `TurNative.createInstance(runtimeHandle, frameLoop, pluginId, instance, baseColorArgb): long`
///
/// Spawns an isolated **renderer-less** VIEW instance for the given
/// `pluginId` — the INITIALIZE half of tur's two-phase lifecycle (#215).
/// The plugin id is stamped into the instance's per-instance data slot at
/// build time (via `TurAppBuilder::instance_data`) so `ease:*` rows can
/// resolve the calling plugin — without trusting a rut argument. No
/// surface is involved: the Kotlin host ATTACHES one later via
/// [`Java_..._TurNative_attachInstance`] (`surfaceCreated`).
///
/// The instance is assigned the shared `ease-plugin-view` worker pool
/// (filed at `createRuntime`). Headless backend instances are NOT spawned
/// through this symbol — the backend crate's `spawn_headless` calls
/// `ops::create_instance` directly (it additionally stamps the per-instance
/// RPC bus and picks the backend pool).
///
/// `instance` is the storage's `plugin_storage_id` for edit-mode views
/// (stamped as `PluginInstance::Some`); pass an empty string for
/// create-mode setup views (`None`).
///
/// `baseColorArgb` is the renderer's base (background) color (Android
/// ARGB int — tur #217): what shows in any frame whose content doesn't
/// cover the viewport — notably the first post-attach present. The Kotlin
/// caller passes the Compose theme's surface color.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_createInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    frame_loop: tur_android::JObject,
    plugin_id: tur_android::JString,
    instance: tur_android::JString,
    base_color_argb: tur_android::jint,
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
    let base_color = Color::rgba(
        ((base_color_argb >> 16) & 0xFF) as u8,
        ((base_color_argb >> 8) & 0xFF) as u8,
        (base_color_argb & 0xFF) as u8,
        ((base_color_argb >> 24) & 0xFF) as u8,
    );
    let view_pool = view_pool_for(runtime_handle);
    tur_android::ops::create_instance(
        &mut env,
        runtime_handle,
        frame_loop,
        move |builder| {
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
        },
        base_color,
    )
}

/// `TurNative.attachInstance(handle, surface, width, height, dpr)` — the
/// ATTACH half of tur's two-phase lifecycle (#215). Call from
/// `surfaceCreated`, where the `Surface` is guaranteed valid. The attach
/// op (FIFO-ordered behind the instance build, so the instance exists
/// when it runs) acquires the `ANativeWindow`, performs the wgpu
/// surface/adapter/device init, and hands the renderer to the engine. On
/// failure the instance stays renderer-less and attachable again. Pair
/// with [`Java_..._TurNative_detachInstance`]; the pair is repeatable —
/// a re-created surface re-attaches without rebuilding the module.
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
/// ref) while the instance keeps running (module, capabilities, events) and
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
    drop_pools(handle);
    tur_android::ops::destroy_runtime(handle)
}

// ---------------------------------------------------------------------------
// EasePluginBridge
// ---------------------------------------------------------------------------

/// Resolve the [`ease_client_backend::ctx::BackendContext`] for a Kotlin-passed backend
/// handle — the same opaque handle the JSON bridge's `{ handle }` envelope
/// uses (`Bridge.getBackendId()`). Returns `None` (after logging) when the
/// handle is unknown: a never-created or already-destroyed backend.
fn backend_cx_for(
    backend_handle: tur_android::jlong,
    caller: &str,
) -> Option<ease_client_backend::ctx::BackendContext> {
    let cx = ease_client_backend::bridge::handle_table::get_backend(backend_handle as u64)
        .map(|b| b.get_context().clone());
    if cx.is_none() {
        tracing::error!(
            "{caller}: backend handle {backend_handle} not found — was Bridge.initialize() \
             called (and not destroy()ed)?"
        );
    }
    cx
}

/// `EasePluginBridge.createRuntime(context, backendHandle): long` —
/// builds the shared tur runtime once, with the Ease plugin set registered
/// on it. View instances are spawned from it via `TurNative.createInstance`;
/// headless backends spawn Rust-side (`PluginEngineHost::spawn_headless`).
/// Allocates the two capped shared worker pools and files them under the
/// returned runtime handle. `backendHandle` binds the engine to a specific
/// backend instance — the `ease:*` rows of every spawned instance resolve
/// their DB/KV/RPC services through it (see
/// [`crate::plugin_runtime::PluginBackendCx`]); an invalid handle fails the
/// call (returns 0).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
    backend_handle: tur_android::jlong,
) -> tur_android::jlong {
    use ease_tur_rpc::TurRpcBusPlugin;
    use tur_animation::TurAnimationPlugin;
    use tur_engine::TurStdPlugin;

    use crate::plugin_runtime::EaseMusicPlugin;

    // Bind this engine (and every instance spawned from it) to the backend
    // instance the Kotlin host named. Without it the `ease:*` rows would
    // be unbound, so an unknown handle is a hard failure.
    let Some(ease_cx) = backend_cx_for(backend_handle, "createRuntime") else {
        return 0;
    };

    // Cache global refs to the Kotlin host classes (EaseOauthHost /
    // EaseThemesHost) so the tur worker thread can call static methods on
    // them — the worker attaches with the system ClassLoader and can't
    // find_class app classes. Must run here (main thread, app ClassLoader).
    if let Err(e) = crate::plugin_runtime::host_cache::cache_host_classes(&mut env) {
        tracing::warn!("host_cache: {e}");
    }

    let pools = PluginWorkerPools::new();
    let pools_for_builder = (pools.backend.clone(), pools.view.clone());

    let runtime = tur_android::ops::create_runtime(&mut env, context, move |builder| {
        // The Http capability (NativeHttp over tur-android's own tokio
        // runtime) + clipboard + fonts register in the AndroidRuntime
        // defaults BEFORE this configure closure runs — we layer the
        // plugin set + pools + the CJK font loader on top.
        let (backend, view) = pools_for_builder;
        builder
            // CJK fallback fonts: registered directly (see plugin_runtime::
            // fonts.rs) so plugin-view text never depends on the device's
            // fonts.xml quirks. Overrides tur's default NativeFontLoader.
            .font_loader(std::sync::Arc::new(
                crate::plugin_runtime::fonts::EaseFontLoader,
            ))
            .worker_pool(backend)
            .worker_pool(view)
            .plugin(TurStdPlugin)
            .plugin(TurAnimationPlugin)
            .plugin(TurRpcBusPlugin::new(None))
            .plugin(EaseMusicPlugin::new(ease_cx))
    });
    if runtime != 0 {
        file_pools(runtime, pools);
    }
    runtime
}

// ============================================================================
// NDK asset FFI lives in [`engine`] — `bindPluginRuntime` below uses its
// `aasset_manager_from_java` to stash the raw `AAssetManager` pointer.
// ============================================================================

/// `EasePluginBridge.bindPluginRuntime(backendHandle, runtimeHandle, assetManager)` —
/// hand the (already-created) tur runtime handle to the named backend
/// instance so `plugin.list` can register module sources on it, and stash
/// the raw `AAssetManager` pointer for reading bundled plugin zips during
/// `plugin.bootstrap`. The AssetManager object is owned by the application
/// Context for the app lifetime, so the raw pointer stays valid. Call once
/// right after `createRuntime` (the Kotlin `KeepBackendService` does it as
/// part of its explicit start sequence) — attaching the engine host also
/// triggers the first backend reload (scan + spawn + wire, Rust-side).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_bindPluginRuntime(
    env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    backend_handle: tur_android::jlong,
    runtime_handle: tur_android::jlong,
    asset_manager: tur_android::JObject,
) {
    let Some(cx) = backend_cx_for(backend_handle, "bindPluginRuntime") else {
        return;
    };
    let mgr = unsafe {
        crate::plugin_runtime::engine::aasset_manager_from_java(
            env.get_raw() as *mut std::ffi::c_void,
            asset_manager.as_raw() as *mut std::ffi::c_void,
        )
    };
    if mgr.is_null() {
        tracing::error!("bindPluginRuntime: AAssetManager_fromJava returned null");
        return;
    }
    crate::plugin_runtime::engine::attach(
        &cx,
        crate::plugin_runtime::engine::TurEngineHost::new(runtime_handle, mgr as usize),
    );
    tracing::info!(
        "bindPluginRuntime: runtime handle {runtime_handle} + asset manager bound \
         (backend handle {backend_handle})"
    );
}

/// `EasePluginBridge.unbindPluginRuntime(backendHandle, runtimeHandle)` —
/// the teardown counterpart of [`Java_..._EasePluginBridge_bindPluginRuntime`]:
/// compare-and-set detaches the engine host (runtime handle + asset
/// manager) the backend's plugin manager holds — a stale stop can never
/// clobber a newer binding — and, when the binding was ours, tears down
/// every Rust-owned headless backend instance + its service RPC entry
/// (stale `RpcClient`s must not survive into storage dispatch). Call
/// BEFORE `TurNative.destroyRuntime` — afterwards the scans would keep
/// registering module sources against a destroyed runtime and every
/// `plugin.list` source handle would silently come back 0.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_unbindPluginRuntime(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    backend_handle: tur_android::jlong,
    runtime_handle: tur_android::jlong,
) {
    let Some(cx) = backend_cx_for(backend_handle, "unbindPluginRuntime") else {
        return;
    };
    let detached = cx.plugin_manager().detach_engine_host(runtime_handle);
    if let Some(host) = detached {
        ease_client_backend::services::plugin_manager::teardown_headless_backends(&cx, host.as_ref());
        tracing::info!(
            "unbindPluginRuntime: runtime handle {runtime_handle} cleared \
             (backend handle {backend_handle})"
        );
    }
}
