//! Cache global `jclass` refs to Kotlin host classes so the tur worker
//! thread — which attaches to the JVM with the **system** ClassLoader and
//! therefore can't `find_class` app classes — can call static methods on them.
//!
//! [`cache_host_classes`] is called once from `createRuntime` (the main
//! thread, where `JNIEnv::find_class` resolves app classes). It leaks two
//! global refs (`EaseThemesHost`, `EaseOauthHost`) and stores the raw
//! pointers as `usize` (raw pointers are not `Send`; `usize` is). The worker
//! thread reconstructs a `JClass` via `JClass::from_raw` for each upcall.
//!
//! Non-Android targets compile as empty.

#![cfg(target_os = "android")]

use std::sync::OnceLock;

use jni::JNIEnv;

static THEME_HOST_CLASS: OnceLock<usize> = OnceLock::new();
static OAUTH_HOST_CLASS: OnceLock<usize> = OnceLock::new();
static STORAGE_HOST_CLASS: OnceLock<usize> = OnceLock::new();

/// Cache global refs to `EaseThemesHost` + `EaseOauthHost` +
/// `EaseStorageHost`. Call from the main thread (e.g. `createRuntime`),
/// where `find_class` can resolve app classes. Idempotent; leaks the
/// global refs for the app lifetime.
pub fn cache_host_classes(env: &mut JNIEnv<'_>) -> Result<(), String> {
    cache_one(env, "com/kutedev/easemusicplayer/turintegration/EaseThemesHost", &THEME_HOST_CLASS)?;
    cache_one(env, "com/kutedev/easemusicplayer/turintegration/EaseOauthHost", &OAUTH_HOST_CLASS)?;
    cache_one(env, "com/kutedev/easemusicplayer/turintegration/EaseStorageHost", &STORAGE_HOST_CLASS)?;
    Ok(())
}

fn cache_one(env: &mut JNIEnv<'_>, name: &str, slot: &'static OnceLock<usize>) -> Result<(), String> {
    if slot.get().is_some() {
        return Ok(());
    }
    let cls = env
        .find_class(name)
        .map_err(|e| format!("host_cache: find_class({name}): {e}"))?;
    let global = env
        .new_global_ref(cls)
        .map_err(|e| format!("host_cache: new_global_ref({name}): {e}"))?;
    let raw = global.as_raw() as usize;
    std::mem::forget(global);
    let _ = slot.set(raw);
    Ok(())
}

/// Raw global `jclass` for `EaseThemesHost`, or `None` before
/// [`cache_host_classes`] has run.
pub fn theme_host_class() -> Option<jni::sys::jclass> {
    THEME_HOST_CLASS.get().map(|r| *r as jni::sys::jclass)
}

/// Raw global `jclass` for `EaseOauthHost`, or `None` before
/// [`cache_host_classes`] has run.
pub fn oauth_host_class() -> Option<jni::sys::jclass> {
    OAUTH_HOST_CLASS.get().map(|r| *r as jni::sys::jclass)
}

/// Raw global `jclass` for `EaseStorageHost`, or `None` before
/// [`cache_host_classes`] has run.
pub fn storage_host_class() -> Option<jni::sys::jclass> {
    STORAGE_HOST_CLASS.get().map(|r| *r as jni::sys::jclass)
}
