package com.kutedev.easemusicplayer.turintegration

import java.io.Closeable
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur runtime — the shared, created-once substrate that
 * isolated [TurInstance]s are spawned from.
 *
 * Built once via [EasePluginBridge.createRuntime] (which calls into
 * `libease_client_android.so` with the Ease plugin set). From a runtime,
 * spawn isolated VIEW instances via [createInstance] (renderer-less; attach
 * an `android.view.Surface` later via [TurInstance.attach]). Headless
 * backend instances are spawned Rust-side (the backend crate's
 * `spawn_headless`) — they never pass through Kotlin. Multiple instances
 * share the runtime's fonts/clock/capabilities/plugins while keeping fully
 * isolated module state.
 */
class TurRuntime(
    handle: Long,
) : Closeable {

    private val handleCell: AtomicLong = AtomicLong(handle)
    /** The opaque native handle (for advanced embedders / passing to [TurNative]). */
    val handle: Long get() = handleCell.get()

    /**
     * Live VIEW instances keyed by `"<pluginId>/<instance>"`. See
     * [createInstance] — at most one live instance per logical view.
     * Rust-spawned headless instances are not tracked here.
     */
    private val liveViews = ConcurrentHashMap<String, TurInstance>()

    /**
     * Register a module rut source on the runtime's shared registry and
     * return its opaque handle. Load it into any instance of this runtime
     * via [TurInstance.loadModule]. Pair with [releaseModuleSource].
     *
     * Sources created on the Rust side (the plugin scan's `plugin.list`)
     * can be passed around as raw handles — no JNI string crossing at all.
     */
    fun registerModuleSource(js: String): Long {
        check(handle != 0L) { "runtime destroyed" }
        return TurNative.registerModuleSource(handle, js)
    }

    /** Drop a registered module source. Idempotent; safe after [close]. */
    fun releaseModuleSource(sourceHandle: Long) {
        if (handle == 0L || sourceHandle == 0L) return
        TurNative.releaseModuleSource(handle, sourceHandle)
    }

    /**
     * Spawn an isolated engine instance (**renderer-less** — the initialize
     * half of the two-phase lifecycle) and return it. Shares this runtime's
     * fonts/clock/capabilities/plugins; gets its own JS realm + element
     * tree. Attach an `android.view.Surface` later via
     * [TurInstance.attach] (from `surfaceCreated`); a never-attached
     * instance is simply headless. [pluginId] is stamped into the
     * instance's per-instance data slot so `ease:*` bridge fns resolve the
     * calling plugin from Rust, not from a JS argument. [instance] is the
     * storage's `plugin_storage_id` for edit-mode views (`null` for create
     * mode) — surfaced to rut as `ease.context.storage_id()`.
     * [baseColorArgb] (Android ARGB int) is the renderer's base
     * (background) color, applied to every surface this instance attaches
     * — the color shown in any frame whose content doesn't cover the
     * viewport (pass `MaterialTheme.colorScheme.surface` so it matches the
     * loading panel).
     *
     * Returns as soon as the native handle exists — the heavy build
     * (worker handshake, plugin registration) runs on the native tur-host
     * thread, so the instance becomes usable asynchronously. A native
     * build failure logs to logcat (no exception here) and later ops on
     * the returned instance become no-ops.
     */
    fun createInstance(
        pluginId: String,
        instance: String? = null,
        baseColorArgb: Int,
    ): TurInstance {
        check(handle != 0L) { "runtime destroyed" }
        // ONE live view instance per (pluginId, instance). A rebind of the
        // same logical view (nav-transition re-compose, rescan key churn,
        // a double-push that slips through) MUST tear its predecessor down
        // first: two live attaches have raced the same recycled
        // ANativeWindow and wgpu aborts the whole process on
        // ERROR_NATIVE_WINDOW_IN_USE_KHR (the play-counts crash). The
        // predecessor's close op is FIFO-queued HERE — ahead of the new
        // instance's build/attach — so the old holder of the window can
        // never outlive the new surface attach.
        val viewKey = "$pluginId/$instance"
        liveViews.remove(viewKey)?.close()
        val frameLoop = FrameLoop()
        val h = TurNative.createInstance(
            handle,
            frameLoop,
            pluginId,
            instance ?: "",
            baseColorArgb,
        )
        check(h != 0L) { "createInstance returned 0 (see logcat)" }
        val inst = TurInstance(h, frameLoop)
        liveViews[viewKey] = inst
        return inst
    }

    /** Drop the runtime and free native resources. Destroy all instances first. Idempotent. */
    override fun close() {
        liveViews.clear()
        val h = handleCell.getAndSet(0L)
        if (h != 0L) {
            TurNative.destroyRuntime(h)
        }
    }

    @Suppress("REM_ELLIPSIS")
    protected fun finalize() {
        close()
    }
}
