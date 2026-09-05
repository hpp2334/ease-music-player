package com.kutedev.easemusicplayer.turintegration

/**
 * JNI upcall target for the Rust `ease.context.notifyChange` /
 * `ease.context.createStorage` host methods.
 *
 * The `ease:context` bridges (in
 * `rust-libs/.../plugin_runtime/context_bridge.rs`) resolve the process
 * `JavaVM` via `ndk_context` and call
 * `EaseStorageHost.notifyChange(pluginId, instance)` /
 * `EaseStorageHost.storageCreated(pluginId, instance)` as static methods from
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

    /**
     * `ease:context.createStorage(pluginId, instance)` upcall entry. Fired
     * when a plugin backend registers a newly-persisted instance (e.g. the
     * WebDAV plugin's `webdav:connect`) — the host reloads its storage list
     * and notifies the create form so it can pop.
     */
    @JvmStatic
    fun storageCreated(pluginId: String, instance: String) {
        val h = handler ?: run {
            android.util.Log.w("EaseStorageHost", "storageCreated ignored: handler not installed")
            return
        }
        h.storageCreated(pluginId, instance)
    }
}
