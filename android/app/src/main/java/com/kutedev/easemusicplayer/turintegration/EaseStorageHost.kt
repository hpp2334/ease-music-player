package com.kutedev.easemusicplayer.turintegration

/**
 * JNI upcall target for the Rust `ease:context.disconnect` host module.
 *
 * The `ease:context.disconnect()` bridge (in
 * `rust-libs/.../plugin_runtime/context_bridge.rs`) resolves the process
 * `JavaVM` via `ndk_context` and calls
 * `EaseStorageHost.disconnect(pluginId, instance)` as a static method from
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
     * `ease:context.disconnect(pluginId, instance)` upcall entry. Finds the
     * storage row matching `(pluginId, pluginStorageId == instance)` and
     * removes it; the resulting `pluginDisconnectedEvent` lets the
     * host-side UI pop back from the edit form.
     */
    @JvmStatic
    fun disconnect(pluginId: String, instance: String) {
        val h = handler ?: run {
            android.util.Log.w("EaseStorageHost", "disconnect ignored: handler not installed")
            return
        }
        h.disconnect(pluginId, instance)
    }
}
