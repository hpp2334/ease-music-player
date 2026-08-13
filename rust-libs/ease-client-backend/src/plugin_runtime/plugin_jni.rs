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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_loadModule(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
    js: tur_android::JString,
) {
    tur_android::ops::load_module(&mut env, handle, js)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_pump(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) -> tur_android::jint {
    tur_android::ops::pump(handle)
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

/// `TurNative.createInstance(runtimeHandle, surface, w, h, dpr, frameLoop, pluginId, instance): long`
///
/// Spawns an isolated rendering instance for the given `pluginId`. The
/// plugin id is stamped into the instance's per-instance data slot at
/// build time (via `TurAppBuilder::instance_data`) so `ease:*` bridge fns
/// can resolve the calling plugin via `extract_js_ctx` + `data::<PluginId>()`
/// — without trusting a JS argument.
///
/// `instance` is the storage's `plugin_storage_id` for edit-mode views
/// (stamped as [`PluginInstance::Some`]); pass an empty string for
/// create-mode setup views (stamped as [`PluginInstance::None`]).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_createInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    surface: tur_android::JObject,
    width: tur_android::jint,
    height: tur_android::jint,
    dpr: tur_android::jdouble,
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
    tur_android::ops::create_instance(
        &mut env,
        runtime_handle,
        surface,
        width,
        height,
        dpr,
        frame_loop,
        move |builder| {
            builder.instance_data(move |cx| {
                cx.define::<crate::plugin_runtime::PluginId>(
                    crate::plugin_runtime::PluginId::new(pid.clone()),
                );
                cx.define::<crate::plugin_runtime::PluginInstance>(
                    crate::plugin_runtime::PluginInstance(instance_opt.clone()),
                );
            })
        },
    )
}

/// `TurNative.createHeadlessInstance(runtimeHandle, frameLoop, pluginId): long`
///
/// Headless variant — same per-instance `PluginId` stamping as
/// [`Java_..._TurNative_createInstance`], but no surface / renderer.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_createHeadlessInstance(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
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
    tur_android::ops::create_headless_instance(
        &mut env,
        runtime_handle,
        frame_loop,
        move |builder| {
            builder.instance_data(move |cx| {
                cx.define::<crate::plugin_runtime::PluginId>(
                    crate::plugin_runtime::PluginId::new(pid.clone()),
                );
                cx.define::<crate::plugin_runtime::PluginInstance>(
                    crate::plugin_runtime::PluginInstance(None),
                );
            })
        },
    )
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

/// `EasePluginBridge.createRuntime(env, context): long` — builds the shared
/// tur runtime once, with the Ease plugin set registered on it. Instances
/// (one per TurView, or a headless one for a service plugin) are spawned from
/// it via `TurNative.createInstance` / `createHeadlessInstance`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
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

    tur_android::ops::create_runtime(&mut env, context, |builder| {
        // tur's engine core is tokio-free (since the drop-tokio refactor); the
        // embedder must hand NativeHttp a Handle onto a runtime it owns + keeps
        // alive for the engine's lifetime. We use the shared ease-client-tokio
        // runtime (same one the backend + JsStorageBackend spawn onto). The
        // builder's capability() takes a closure that may receive the engine's
        // AsyncPluginContext; NativeHttp needs only the tokio Handle.
        let handle = ease_client_tokio::tokio_runtime().handle().clone();
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
    let Some(rpc) = tur_android::ops::with_app(instance_handle, |app| {
        ease_tur_rpc::RpcClient::wire(app)
    }) else {
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
