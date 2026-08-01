package com.kutedev.easemusicplayer.turintegration

import android.os.Handler
import android.os.Looper
import android.view.Choreographer

/**
 * Frame scheduler the native `LoopDriver` drives.
 *
 * The engine decides when it wants the next wake-up verdict and calls one
 * of `scheduleVsync` / `scheduleDelayed` / `cancel` back through JNI. When
 * the wake-up fires, [FrameLoop] invokes [onWake] (which [TurInstance] wires
 * to the engine's `pump`), completing the loop.
 *
 * Lives on the main looper (where `SurfaceHolder.Callback` and input
 * dispatch arrive), matching the single-threaded assumption the native
 * side relies on.
 */
class FrameLoop {
    private val handler = Handler(Looper.getMainLooper())
    private var frameCallback: Choreographer.FrameCallback? = null
    private var delayedToken: Runnable? = null

    /** Fired when a scheduled wake-up is due. [TurInstance] sets this to `pump`. */
    var onWake: (() -> Unit)? = null

    /** Optional callback fired after [onWake] in each wake-up. */
    var onAfterPump: (() -> Unit)? = null

    /** Schedule a wake on the next display frame (Android `Choreographer`). */
    fun scheduleVsync() {
        if (frameCallback != null) return
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                frameCallback = null
                onWake?.invoke()
                onAfterPump?.invoke()
            }
        }
        frameCallback = cb
        Choreographer.getInstance().postFrameCallback(cb)
    }

    /** Schedule a wake [delayMs] milliseconds from now. */
    fun scheduleDelayed(delayMs: Long) {
        if (delayedToken != null) return
        val r = Runnable {
            delayedToken = null
            onWake?.invoke()
            onAfterPump?.invoke()
        }
        delayedToken = r
        handler.postDelayed(r, delayMs.coerceAtLeast(1))
    }

    /** Cancel any pending wake-up (the engine went idle). */
    fun cancel() {
        frameCallback?.let { Choreographer.getInstance().removeFrameCallback(it) }
        frameCallback = null
        delayedToken?.let { handler.removeCallbacks(it) }
        delayedToken = null
    }
}
