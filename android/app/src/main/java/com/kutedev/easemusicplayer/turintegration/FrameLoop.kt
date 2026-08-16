package com.kutedev.easemusicplayer.turintegration

import android.os.Handler
import android.os.Looper
import android.view.Choreographer

/**
 * Frame scheduler the native loop driver drives.
 *
 * The engine decides when it wants the next display frame and calls
 * `scheduleVsync` back through JNI (Choreographer-backed). When the wake-up
 * fires, [FrameLoop] invokes [onVsync] (which [TurInstance] wires to the
 * engine's `pump`), completing the loop:
 *
 * ```
 * engine run_loop() → schedule == Vsync → FrameLoop.scheduleVsync()
 *   → Choreographer frame → FrameLoop.onVsync() → nativePump() → engine run_loop() → …
 * ```
 *
 * Separately, worker→main messages and main-loop tasks that merely need the
 * main loop *polled* (no display frame) go through [requestPump] — a
 * coalesced main-Handler post that invokes [onPump] (the engine's
 * `pumpMessages`: poll the loop, do NOT fire a vsync). This split is what
 * lets an idle instance park at 0% CPU: without it, every `FrameOutcome`
 * (shipped after each engine pump) would re-arm the Choreographer and
 * ping-pong the whole engine (a full flush per pump) at display refresh
 * rate forever — even fully idle.
 *
 * Lives on the main looper (where `SurfaceHolder.Callback` and input dispatch
 * arrive), matching the single-threaded assumption the native side relies on.
 *
 * [onVsync] / [onPump] are settable (default `null`) so a [FrameLoop] can be
 * constructed before the instance handle exists and wired up by
 * [TurInstance] afterwards — the runtime needs a `FrameLoop` to hand to
 * native `createInstance`, but the `pump` target only exists once
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
     * Whether the engine's focused element is an editable text field, pushed
     * from native via [onFocusChanged] (the engine emits a focus-change event
     * each time the focused element / caret rect changes). Read by the
     * Compose integration's per-frame IME sync ([onAfterPump]) to decide
     * whether to raise the soft keyboard — without a JNI round-trip per frame.
     */
    var focusedIsEditable: Boolean = false
        private set

    /**
     * Optional callback fired after [onVsync] / [onPump] in each wake-up.
     * Runs on the main looper, same as the wake callbacks. `null` by default.
     */
    var onAfterPump: (() -> Unit)? = null

    /**
     * Called from native (via JNI) when the engine's focused-element state
     * changes. Stores the editable flag so [onAfterPump]'s IME sync can read
     * it without a JNI round-trip per frame. The engine is the source of
     * truth — Kotlin never queries focus state, only consumes this push.
     */
    fun onFocusChanged(isEditable: Boolean) {
        focusedIsEditable = isEditable
    }

    /**
     * Schedule a wake on the next display frame (Android `Choreographer`).
     *
     * `Choreographer.getInstance()` is **thread-local** — it returns the
     * calling thread's Choreographer, which only exists on threads with a
     * `Looper`. The engine worker thread has no Looper, so calling this from
     * the worker would get a wrong/no Choreographer and the frame callback
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
     * worker→main messages or main-loop tasks need processing while the
     * engine is otherwise idle (`schedule == Idle`) — polling must not arm
     * the Choreographer, or the engine would burn a full frame per message
     * at display refresh rate.
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
