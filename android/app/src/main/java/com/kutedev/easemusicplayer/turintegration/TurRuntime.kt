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
) : Closeable {

    private val handleCell: AtomicLong = AtomicLong(handle)
    /** The opaque native handle (for advanced embedders / passing to [TurNative]). */
    val handle: Long get() = handleCell.get()

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
        val h = TurNative.createHeadlessInstance(handle, frameLoop, pluginId)
        check(h != 0L) { "createHeadlessInstance returned 0 (see logcat)" }
        return TurInstance(h, frameLoop)
    }

    /** Drop the runtime and free native resources. Destroy all instances first. Idempotent. */
    override fun close() {
        val h = handleCell.getAndSet(0L)
        if (h == 0L) return
        TurNative.destroyRuntime(h)
    }

    @Suppress("REM_ELLIPSIS")
    protected fun finalize() {
        close()
    }
}
