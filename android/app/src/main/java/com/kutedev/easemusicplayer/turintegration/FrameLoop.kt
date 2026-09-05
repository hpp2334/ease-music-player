package com.kutedev.easemusicplayer.turintegration

import android.os.Handler
import android.os.Looper
import android.view.Choreographer

/**
 * Frame scheduler the native host thread drives.
 *
 * The engine decides when it wants the next display frame and calls
 * `scheduleVsync` back through JNI (Choreographer-backed). When the wake-up
 * fires, [FrameLoop] invokes [onVsync] (which [TurInstance] wires to the
 * engine's `pump`), completing the loop:
 *
 * ```
 * engine loop (TurAppLooper.run) → schedule == Vsync → FrameLoop.scheduleVsync()
 *   → Choreographer frame (main) → FrameLoop.onVsync() → nativePump()
 *   → posted onto the native tur-host thread → engine loop → …
 * ```
 *
 * The **frame itself runs on the native tur-host thread** — `nativePump`
 * only posts the work there, so the Choreographer callback returns
 * immediately and the Android main thread stays free (GPU encode/present,
 * layout application, and input handling all happen off-main).
 *
 * Separately, worker→host messages and host-loop tasks that merely need the
 * loop *polled* (no display frame) go through [requestPump] — a coalesced
 * main-Handler post that invokes [onPump] (the engine's `pumpMessages`:
 * poll the loop, do NOT fire a vsync). This split is what lets an idle
 * instance park at 0% CPU: without it, every `FrameOutcome` (shipped after
 * each engine pump) would re-arm the Choreographer and ping-pong the whole
 * engine (a full flush per pump) at display refresh rate forever — even
 * fully idle. (The primary idle wake is now an even shorter path: native
 * posts the poll directly onto the tur-host thread without touching this
 * loop at all — [requestPump] remains as the fallback for paths that come
 * through Kotlin.)
 *
 * Lives on the main looper (where `SurfaceHolder.Callback` and input
 * dispatch arrive); its callbacks merely post work to the native side.
 *
 * [onVsync] / [onPump] / [onAfterPump] are settable (default `null`) so a
 * [FrameLoop] can be constructed before the instance handle exists and wired
 * up by [TurInstance] afterwards — the runtime needs a `FrameLoop` to hand
 * to native `createInstance`, but the `pump` target only exists once
 * `createInstance` returns.
 */
class FrameLoop {
    private val handler = Handler(Looper.getMainLooper())
    private var frameCallback: Choreographer.FrameCallback? = null
    private var pumpPosted = false

    /** Fired when a scheduled display frame is due (Choreographer). */
    var onVsync: (() -> Unit)? = null

    /**
     * Fired when the main loop needs polling without a display frame —
     * a coalesced Handler post requested by native (`requestPump`).
     */
    var onPump: (() -> Unit)? = null

    /**
     * Whether the engine's focused element is an editable text field (an
     * active text-input / IME session), pushed from native via
     * [onTextInputChanged] (the engine emits a shell text-input state
     * change each time the focused editable / caret rect changes). Read by
     * the per-frame IME sync ([onAfterPump]) to decide whether to raise
     * the soft keyboard — without a JNI round-trip per frame.
     */
    var textInputActive: Boolean = false
        private set

    /**
     * Optional callback fired after [onVsync] / [onPump] in each wake-up
     * (and after a [onTextInputChanged] state change — see there). The
     * host sets this to sync the Android soft-keyboard / IME with the
     * engine's text-input state (read [textInputActive], then
     * `showSoftInput` / `hideSoftInput`). Runs on the main looper, same as
     * the wake callbacks. State-gated consumers are safe even though the
     * posted native frame may still be in flight — the text-input flag is
     * reconciled again on the next change or tick. `null` by default.
     */
    var onAfterPump: (() -> Unit)? = null

    /**
     * Called from native (via JNI, on the tur-host thread where frames run)
     * when the engine's text-input session state changes. Stores the
     * editable flag so [onAfterPump]'s IME sync can read it without a JNI
     * round-trip per frame. The engine is the source of truth — Kotlin
     * never queries focus state, only consumes this push.
     *
     * On a state CHANGE it also schedules a reconcile ([onAfterPump] posted
     * to the main handler): since frames run off-main, the engine may go
     * idle right after pushing a change, and the keyboard transition must
     * not wait for a Choreographer tick that never comes. The post's
     * happens-before edge is also what publishes the field write to
     * main-thread readers.
     */
    fun onTextInputChanged(isEditable: Boolean) {
        val changed = textInputActive != isEditable
        textInputActive = isEditable
        if (changed) {
            handler.post { onAfterPump?.invoke() }
        }
    }

    /**
     * Schedule a wake on the next display frame (Android `Choreographer`).
     *
     * `Choreographer.getInstance()` is **thread-local** — it returns the
     * calling thread's Choreographer, which only exists on threads with a
     * `Looper`. The native tur-host thread has no Looper, so calling this
     * from it would get a wrong/no Choreographer and the frame callback
     * would never fire. Hop to the main thread first when called off-main.
     */
    fun scheduleVsync() {
        if (Looper.getMainLooper().thread != Thread.currentThread()) {
            handler.post { scheduleVsync() }
            return
        }
        if (frameCallback != null) return // already armed
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                frameCallback = null
                onVsync?.invoke()
                onAfterPump?.invoke()
            }
        }
        frameCallback = cb
        Choreographer.getInstance().postFrameCallback(cb)
    }

    /**
     * Request a main-loop poll WITHOUT a display frame: a coalesced
     * main-Handler post that invokes [onPump]. Called from native when
     * worker→host messages or host-loop tasks need processing while the
     * engine is otherwise idle (`schedule == Idle`) — polling must not arm
     * the Choreographer, or the engine would burn a full frame per message
     * at display refresh rate. (The primary idle wake is a direct native
     * post onto the tur-host thread; this JNI path is the fallback for
     * wakes that route through Kotlin.)
     */
    fun requestPump() {
        // May be called from any thread (the engine worker via JNI); the
        // handler queue's happens-before edge covers the flag hand-off.
        synchronized(this) {
            if (pumpPosted) return // already posted (coalesce)
            pumpPosted = true
        }
        handler.post {
            synchronized(this) { pumpPosted = false }
            onPump?.invoke()
            onAfterPump?.invoke()
        }
    }

    /** Schedule a wake [delayMs] milliseconds from now. */
    fun scheduleDelayed(delayMs: Long) {
        val r = Runnable {
            onPump?.invoke()
            onAfterPump?.invoke()
        }
        handler.postDelayed(r, delayMs.coerceAtLeast(1))
    }

    /** Cancel any pending wake-up (the engine went idle). */
    fun cancel() {
        frameCallback?.let { Choreographer.getInstance().removeFrameCallback(it) }
        frameCallback = null
        synchronized(this) { pumpPosted = false }
        handler.removeCallbacksAndMessages(null)
    }
}
