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
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_TurNative_focusedIsEditable(
    _env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    handle: tur_android::jlong,
) -> tur_android::jboolean {
    tur_android::ops::focused_is_editable(handle)
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

/// `EasePluginBridge.createEngine(env, context, surface, w, h, dpr, frameLoop): long`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createEngine(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
    surface: tur_android::JObject,
    width: tur_android::jint,
    height: tur_android::jint,
    dpr: tur_android::jdouble,
    frame_loop: tur_android::JObject,
) -> tur_android::jlong {
    use tur_animation::TurAnimationPlugin;
    use tur_engine::{TurClipboardPlugin, TurStdPlugin};
    use tur_net_native::TurNetPlugin;

    use crate::plugin_runtime::EaseMusicPlugin;

    tur_android::ops::create_with_plugins(
        &mut env,
        context,
        surface,
        width,
        height,
        dpr,
        frame_loop,
        |builder| {
            builder
                .plugin(TurStdPlugin)
                .plugin(TurAnimationPlugin)
                .plugin(TurClipboardPlugin)
                .plugin(TurNetPlugin)
                .plugin(EaseMusicPlugin)
        },
    )
}
