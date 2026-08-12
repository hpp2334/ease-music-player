package com.kutedev.easemusicplayer.turintegration

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur instance — an isolated JS realm + element tree +
 * renderer (or headless), spawned from a [TurRuntime].
 *
 * Built on top of [TurNative] (the JNI bridge): holds the instance handle,
 * drives frames via a [FrameLoop], and translates Android input into the
 * engine's platform-event stream. Instances are created by
 * [TurRuntime.createInstance] / [TurRuntime.createHeadlessInstance] — not
 * constructed directly.
 *
 * Use [TurView] in Compose rather than this class directly — [TurView] wires
 * surface lifecycle, input dispatch, and the frame loop together.
 *
 * @param handle the opaque native instance pointer. `0` is treated as
 *   "destroyed".
 * @param frameLoop the per-instance scheduler the native loop driver arms
 *   wake-ups against; [pump] is wired to its wake callback.
 */
class TurInstance(
    handle: Long,
    private val frameLoop: FrameLoop,
) : Closeable {

    /** Atomic so the finalizer / close() race is safe even though we're single-threaded. */
    private val handleCell: AtomicLong = AtomicLong(handle)
    private val handle: Long get() = handleCell.get()

    init {
        // Wire the frame loop's wake callback to pump this instance. Done at
        // construction (the instance already exists by the time the handle
        // reaches us) so the first Choreographer tick advances it.
        frameLoop.onWake = { if (handle != 0L) TurNative.pump(handle) }
    }

    /** Evaluate [js] (an ES module) and request a paint. */
    fun loadModule(js: String) {
        check(handle != 0L) { "instance destroyed" }
        TurNative.loadModule(handle, js)
    }

    /** Push a new logical size + dpr (from `SurfaceHolder.Callback.surfaceChanged`). */
    fun resize(width: Int, height: Int, dpr: Double) {
        if (handle == 0L) return
        TurNative.resize(handle, width, height, dpr)
    }

    /** Fire one engine wake (the Choreographer / Handler callback). */
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

    /**
     * Whether the focused element is an editable text field. Read from the
     * [FrameLoop]'s retained value (pushed from native via
     * `FrameLoop.onFocusChanged`), so this is a cheap Kotlin field read — no
     * JNI round-trip.
     */
    fun focusedIsEditable(): Boolean =
        handle != 0L && frameLoop.focusedIsEditable

    /**
     * Push an IME composition event. [kind]: `0=Start`, `1=Update`, `2=End`.
     */
    fun pushIme(kind: Int, text: String) {
        if (handle == 0L) return
        TurNative.pushIme(handle, kind, text)
    }

    /**
     * Install a callback fired after each engine pump (on the main looper,
     * immediately after the frame runs). Pass `null` to clear.
     */
    fun setAfterPump(cb: (() -> Unit)?) {
        frameLoop.onAfterPump = cb
    }

    /** The opaque native handle (for advanced embedders / debugging). */
    fun nativeHandle(): Long = handle

    /** Drop the instance and free native resources. The parent runtime is
     *  unaffected. Idempotent. */
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
