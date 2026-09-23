package com.kutedev.easemusicplayer.turintegration

import android.os.Handler
import android.os.Looper
import android.util.Log
import java.util.concurrent.CopyOnWriteArraySet
import java.util.concurrent.atomic.AtomicLong

/**
 * The single JNI→host fire-and-forget upcall surface: Rust pushes signals
 * here through the static [`onSignal`] (called from
 * `ease-client-android`'s engine host via the cached class ref — no return
 * value ever crosses back; host data flows to Rust as state pushes, not
 * queries).
 *
 * Signals are marshalled onto the main looper and fanned out to registered
 * listeners in emission order. Op codes mirror the Rust
 * `SIGNAL_*` constants (`services/plugin_manager.rs`):
 *
 * - `1` — `PLUGINS_CHANGED`: payload is the new plugin generation. The
 *   installed/enabled set (and possibly the live backend set) changed;
 *   consumers rescan.
 */
object EaseSignalHost {
    const val OP_PLUGINS_CHANGED = 1

    private const val TAG = "EaseSignalHost"

    /** Monotonic count of delivered signals (diagnostics). */
    private val delivered = AtomicLong(0L)

    private val mainHandler = Handler(Looper.getMainLooper())

    /**
     * Thread-safe listener set; registered by long-lived singletons
     * (the `EaseBackend` facade). A listener sees every signal exactly
     * once, on the main looper, in emission order.
     */
    private val listeners = CopyOnWriteArraySet<(Int, String) -> Unit>()

    fun register(listener: (Int, String) -> Unit) {
        listeners += listener
    }

    fun unregister(listener: (Int, String) -> Unit) {
        listeners -= listener
    }

    /** JNI entry — called from an attached native thread. Never throws. */
    @JvmStatic
    fun onSignal(op: Int, payload: String) {
        val n = delivered.incrementAndGet()
        Log.d(TAG, "signal #$n op=$op payload=${payload.take(200)}")
        mainHandler.post {
            for (l in listeners) {
                runCatching { l(op, payload) }
                    .onFailure { Log.e(TAG, "signal listener failed (op=$op)", it) }
            }
        }
    }
}
