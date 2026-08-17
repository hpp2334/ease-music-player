package com.kutedev.easemusicplayer.turintegration

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur runtime — the shared, created-once substrate that
 * isolated [TurInstance]s are spawned from.
 *
 * Built once via [EasePluginBridge.createRuntime] (which calls into
 * `libease_client_backend.so` with the Ease plugin set). From a runtime,
 * create rendering instances via [createInstance] (attached to a
 * `android.view.Surface`) or headless instances via [createHeadlessInstance].
 * Multiple instances share the runtime's fonts/clock/capabilities/plugins
 * while keeping fully isolated JS state.
 */
class TurRuntime(
    handle: Long,
    poolsHandle: Long = 0L,
) : Closeable {

    private val handleCell: AtomicLong = AtomicLong(handle)
    /** The opaque native handle (for advanced embedders / passing to [TurNative]). */
    val handle: Long get() = handleCell.get()

    private val poolsCell: AtomicLong = AtomicLong(poolsHandle)
    /**
     * The opaque native worker-pools handle (from
     * `EasePluginBridge.createPluginWorkerPools`), passed through to every
     * spawned instance. `0` = engine default (one lane thread per instance).
     */
    val poolsHandle: Long get() = poolsCell.get()

    /**
     * Register a JS module source on the runtime's shared registry and
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
     * Spawn an isolated rendering instance attached to [surface] and return it.
     * Shares this runtime's fonts/clock/capabilities/plugins; gets its own JS
     * realm, element tree, and renderer. [pluginId] is stamped into the
     * instance's per-instance data slot so `ease:*` bridge fns resolve the
     * calling plugin from Rust, not from a JS argument. [instance] is the
     * storage's `plugin_storage_id` for edit-mode views (`null` for create
     * mode) — exposed to JS as `ease.context.instance()`.
     */
    fun createInstance(
        surface: android.view.Surface,
        width: Int,
        height: Int,
        dpr: Double,
        pluginId: String,
        instance: String? = null,
    ): TurInstance {
        check(handle != 0L) { "runtime destroyed" }
        val frameLoop = FrameLoop()
        val h = TurNative.createInstance(
            handle,
            poolsHandle,
            surface,
            width,
            height,
            dpr,
            frameLoop,
            pluginId,
            instance ?: "",
        )
        check(h != 0L) { "createInstance returned 0 (see logcat)" }
        return TurInstance(h, frameLoop)
    }

    /**
     * Spawn an isolated headless instance (no surface, no rendering). Runs JS +
     * capabilities + events only. Useful for service plugins (e.g. a JS storage
     * provider) that must stay alive independent of any view. [pluginId] mirrors
     * [`createInstance`]'s identity stamp.
     */
    fun createHeadlessInstance(pluginId: String): TurInstance {
        check(handle != 0L) { "runtime destroyed" }
        val frameLoop = FrameLoop()
        val h = TurNative.createHeadlessInstance(handle, poolsHandle, frameLoop, pluginId)
        check(h != 0L) { "createHeadlessInstance returned 0 (see logcat)" }
        return TurInstance(h, frameLoop)
    }

    /** Drop the runtime and free native resources. Destroy all instances first. Idempotent. */
    override fun close() {
        val h = handleCell.getAndSet(0L)
        if (h != 0L) {
            TurNative.destroyRuntime(h)
        }
        val p = poolsCell.getAndSet(0L)
        if (p != 0L) {
            EasePluginBridge.destroyPluginWorkerPools(p)
        }
    }

    @Suppress("REM_ELLIPSIS")
    protected fun finalize() {
        close()
    }
}
