package com.kutedev.easemusicplayer.turintegration

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur instance — an isolated JS realm + element tree,
 * spawned renderer-less from a [TurRuntime] and attached to an Android
 * [android.view.Surface] later (the two-phase initialize → attach
 * lifecycle; a never-attached instance is simply headless).
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
 *   wake-ups against; [pump] / [pumpMessages] are wired to its wake
 *   callbacks.
 */
class TurInstance(
    handle: Long,
    private val frameLoop: FrameLoop,
) : Closeable {

    /** Atomic so the finalizer / close() race is safe even though we're single-threaded. */
    private val handleCell: AtomicLong = AtomicLong(handle)
    private val handle: Long get() = handleCell.get()

    private var bootAt = 0L
    private var firstPumpLogged = false

    /** Perf instrumentation: stamp instance-spawn completion (TurPerf). */
    fun markBoot() {
        bootAt = android.os.SystemClock.elapsedRealtime()
    }

    /** Perf instrumentation: log time from boot stamp to the first engine pump (TurPerf). */
    fun markFirstPump() {
        if (firstPumpLogged || bootAt == 0L) return
        firstPumpLogged = true
        android.util.Log.d(
            "TurPerf",
            "first pump: ${android.os.SystemClock.elapsedRealtime() - bootAt}ms after loadModule (module eval + mount + render)",
        )
    }

    init {
        // Wire the frame loop's wake callbacks to pump this instance. The
        // native route exists by the time the handle reaches us (the heavy
        // build may still be queued on the tur-host thread — FIFO op order
        // makes any wake that lands first wait behind it). `onVsync` = a
        // display frame was requested (fires the engine's vsync event +
        // polls the loop); `onPump` = messages/tasks need the loop polled
        // WITHOUT a frame (keeps an idle instance at 0% CPU).
        //
        // NOTE: `this.handle` (the property reading the atomic cell) — NOT
        // the constructor parameter. The parameter is captured by these
        // closures and never changes; reading it would keep firing native
        // pumps on a destroyed instance after `close()` (use-after-free in
        // the engine's `pump_loop`).
        frameLoop.onVsync = { if (this.handle != 0L) TurNative.pump(this.handle) }
        frameLoop.onPump = { if (this.handle != 0L) TurNative.pumpMessages(this.handle) }
    }

    /**
     * Attach a rendering surface — the **attach** half of the two-phase
     * lifecycle (call from `surfaceCreated`, where the surface is valid).
     * The native attach op (ordered behind the instance build) performs
     * the wgpu surface/adapter/device init and hands the renderer to the
     * engine. On failure the instance stays renderer-less and attachable
     * again. Pair with [detach]; the pair is repeatable — a re-created
     * surface re-attaches without rebuilding the JS realm.
     */
    fun attach(surface: android.view.Surface, width: Int, height: Int, dpr: Double) {
        if (handle == 0L) return
        TurNative.attachInstance(handle, surface, width.coerceAtLeast(1), height.coerceAtLeast(1), dpr)
    }

    /**
     * Detach the rendering surface — the **detach** half (call from
     * `surfaceDestroyed`). Drops the renderer + releases the native window;
     * the instance keeps running and can [attach] a fresh surface later.
     * Idempotent.
     */
    fun detach() {
        if (handle == 0L) return
        TurNative.detachInstance(handle)
    }

    /**
     * Evaluate a registered module source (a [TurRuntime.registerModuleSource]
     * handle, or one created on the Rust side — `plugin.list`) as an ES
     * module and request a paint. The shared source reaches the engine by
     * refcount — no JNI string copy per load.
     */
    fun loadModule(sourceHandle: Long) {
        check(handle != 0L) { "instance destroyed" }
        check(sourceHandle != 0L) { "invalid module source handle" }
        TurNative.loadModule(handle, sourceHandle)
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
     * Whether an editable text field is focused (an active text-input / IME
     * session). Read from the [FrameLoop]'s retained value (pushed from
     * native via `FrameLoop.onTextInputChanged`), so this is a cheap Kotlin
     * field read — no JNI round-trip.
     */
    fun textInputActive(): Boolean =
        handle != 0L && frameLoop.textInputActive

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
     *  unaffected. Idempotent.
     *
     *  Fire-and-forget (main-thread safe): the destroy op is queued on the
     *  native tur-host thread; a still-in-flight build for this instance is
     *  harmless — the build creates no surface (the two-phase lifecycle), so
     *  whichever op runs second simply finds nothing. A still-attached
     *  surface is detached by the destroy op itself. Use [closeBlocking]
     *  when you need to know teardown finished. */
    override fun close() {
        val h = handleCell.getAndSet(0L)
        if (h == 0L) return
        frameLoop.cancel()
        TurNative.destroy(h)
    }

    /** Close and **block until native teardown settled** — the destroy op ran
     *  on the tur-host thread (instance, renderer, surface dropped; the
     *  module's `start()` cleanup runs on the worker as it winds down).
     *
     *  The fence for disposal-sensitive flows — replaces sleep-based quiesce
     *  heuristics. **Off-main-thread only**: it can wait behind an in-flight
     *  instance build (~hundreds of ms of GPU init), e.g.
     *  `withContext(Dispatchers.Default) { instance.closeBlocking() }`.
     *  Idempotent. */
    fun closeBlocking() {
        val h = handleCell.getAndSet(0L)
        if (h == 0L) return
        frameLoop.cancel()
        TurNative.destroySettled(h)
    }

    @Suppress("REM_ELLIPSIS")
    protected fun finalize() {
        close()
    }
}
