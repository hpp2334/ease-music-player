package com.kutedev.easemusicplayer.turintegration

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur engine instance, identified by its opaque [handle].
 *
 * Built on top of [TurNative] (the JNI bridge): holds the engine handle,
 * drives frames via a [FrameLoop], and translates Android input into the
 * engine's platform-event stream. This class does **not** create the
 * engine — the app builds it via a [TurEngineFactory] (which calls into
 * `libease_client_backend.so`'s `EasePluginBridge.createEngine`) and
 * hands the resulting handle here.
 *
 * Use [TurView] in Compose rather than this class directly.
 */
class TurEngine(
    handle: Long,
    private val frameLoop: FrameLoop,
) : Closeable {

    private val handleCell: AtomicLong = AtomicLong(handle)
    private val handle: Long get() = handleCell.get()

    init {
        frameLoop.onWake = { if (handle != 0L) TurNative.pump(handle) }
    }

    /** Evaluate [js] (an ES module) and request a paint. */
    fun loadModule(js: String) {
        check(handle != 0L) { "engine destroyed" }
        TurNative.loadModule(handle, js)
    }

    /** Push a new logical size + dpr. */
    fun resize(width: Int, height: Int, dpr: Double) {
        if (handle == 0L) return
        TurNative.resize(handle, width, height, dpr)
    }

    /** Fire one engine wake. */
    fun pump() {
        if (handle == 0L) return
        TurNative.pump(handle)
    }

    /** Dispatch a pointer (touch) event. [action] is `MotionEvent.ACTION_*`. */
    fun pushPointer(action: Int, x: Double, y: Double, timeMs: Long) {
        if (handle == 0L) return
        TurNative.pushPointer(handle, action, x, y, timeMs)
    }

    /** Dispatch a key event (browser-style `key`/`code`). */
    fun pushKey(
        key: String,
        code: String,
        action: Int,
        ctrl: Boolean,
        shift: Boolean,
        alt: Boolean,
        meta: Boolean,
    ) {
        if (handle == 0L) return
        TurNative.pushKey(handle, key, code, action, ctrl, shift, alt, meta)
    }

    /** Whether the focused element is an editable text field. */
    fun focusedIsEditable(): Boolean =
        handle != 0L && TurNative.focusedIsEditable(handle)

    /** Push an IME composition event. [kind]: `0=Start`, `1=Update`, `2=End`. */
    fun pushIme(kind: Int, text: String) {
        if (handle == 0L) return
        TurNative.pushIme(handle, kind, text)
    }

    /**
     * Install a callback fired after each engine pump (on the main looper).
     * Pass `null` to clear.
     */
    fun setAfterPump(cb: (() -> Unit)?) {
        frameLoop.onAfterPump = cb
    }

    /** The opaque native handle (for advanced embedders / debugging). */
    fun nativeHandle(): Long = handle

    /** Drop the engine and free native resources. Idempotent. */
    override fun close() {
        val h = handleCell.getAndSet(0L)
        if (h == 0L) return
        frameLoop.cancel()
        TurNative.destroy(h)
    }

    @Suppress("REM_ELLIPSIS")
    protected fun finalize() {
        close()
    }
}
