package com.kutedev.easemusicplayer.turintegration

/**
 * JNI upcall target for the Rust `ease.context.notifyChange` host method.
 *
 * The `ease:context.notifyChange()` bridge (in
 * `rust-libs/.../plugin_runtime/context_bridge.rs`) resolves the process
 * `JavaVM` via `ndk_context` and calls
 * `EaseStorageHost.notifyChange(pluginId, instance)` as a static method from
 * the engine thread. This object is a thin static shell — it delegates to
 * a [StorageHandler] installed once at startup
 * ([MainActivity.onCreate]), which owns the Hilt-injected dependencies
 * (repositories, app scope).
 */
object EaseStorageHost {
    @Volatile
    private var handler: StorageHandler? = null

    /** Install the runtime handler. Called once from `MainActivity.onCreate`. */
    fun install(h: StorageHandler) {
        handler = h
    }

    /**
     * `ease:context.notifyChange(pluginId, instance)` upcall entry. Asks the
     * host to reload its storage list so kv-side changes (an alias rename
     * written by a plugin view, or a removal) propagate to the dashboard +
     * edit page. The args are informational (the reload is full-list); they
     * are kept for parity with the per-instance identity and future
     * targeted refreshes.
     */
    @JvmStatic
    fun notifyChange(pluginId: String, instance: String) {
        val h = handler ?: run {
            android.util.Log.w("EaseStorageHost", "notifyChange ignored: handler not installed")
            return
        }
        h.notifyChange(pluginId, instance)
    }
}
