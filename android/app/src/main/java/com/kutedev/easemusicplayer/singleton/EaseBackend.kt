package com.kutedev.easemusicplayer.singleton

import android.content.Context
import android.util.Log
import com.kutedev.easemusicplayer.turintegration.EaseSignalHost
import com.kutedev.easemusicplayer.turintegration.TurRuntime
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow

/** A fire-and-forget signal pushed by the Rust backend (see `EaseSignalHost`). */
sealed interface BackendSignal {
    /**
     * The plugin set changed (install / uninstall / enable / disable /
     * reload) — payload is the new backend generation. Consumers rescan
     * their plugin-derived state.
     */
    data class PluginsChanged(val generation: Long) : BackendSignal
}

/**
 * The unified Kotlin-side owner of the backend process: it holds the JSON
 * [Bridge] plus the plugin runtime (worker pools + shared tur engine
 * runtime), and drives their **explicit** lifecycle in one place:
 *
 * 1. [start] — requires [Bridge.initialize] to have run (the runtime binds
 *    to that backend instance's handle): pools → runtime →
 *    `bindPluginRuntime` (which also hands Rust the pools + asset manager,
 *    triggering the first Rust-side backend reload). Every step is logged
 *    (logcat + the in-app backend log). Idempotent: a second [start] logs
 *    and returns the live runtime instead of silently rebuilding.
 * 2. [runtimeOrNull] / [runtime] — what UI pages consume; `null` means
 *    "not up (yet)" and callers render their fallback. The [StateFlow]
 *    makes pages self-heal when the owning service (re)starts.
 * 3. [stop] — `unbindPluginRuntime` (while the backend handle is still
 *    resolvable; the unbind also tears down the Rust-owned headless
 *    backends) → [TurRuntime.close]. Idempotent, reason logged.
 * 4. [signals] — the Kotlin fan-out of the Rust backend's fire-and-forget
 *    signals (`EaseSignalHost` → here), typed as [BackendSignal].
 *
 * The only lifecycle driver is [com.kutedev.easemusicplayer.core.KeepBackendService];
 * the heavy logic (which backends run, when they reload) is Rust-side —
 * this facade owns the Android half of the graph and nothing else.
 */
@Singleton
class EaseBackend @Inject constructor(
    private val bridge: Bridge,
) {
    private val lock = Any()

    private var runtimeField: TurRuntime? = null

    /** `null` = not started (or stopped — see the log for the reason). */
    private val _runtime = MutableStateFlow<TurRuntime?>(null)

    /** Observability for UI pages: goes non-null on [start], null on [stop]. */
    val runtime: StateFlow<TurRuntime?> = _runtime.asStateFlow()

    private val _signals = MutableSharedFlow<BackendSignal>(extraBufferCapacity = 32)
    val signals: SharedFlow<BackendSignal> = _signals.asSharedFlow()

    init {
        // The single Rust→Kotlin signal fan-in: EaseSignalHost marshals
        // onto the main looper, we translate op codes into typed events.
        EaseSignalHost.register { op, payload ->
            when (op) {
                EaseSignalHost.OP_PLUGINS_CHANGED -> {
                    val generation = payload.toLongOrNull() ?: -1L
                    _signals.tryEmit(BackendSignal.PluginsChanged(generation))
                }
                else -> Log.w(TAG, "unknown signal op=$op payload=${payload.take(200)}")
            }
        }
    }

    /**
     * Create + bind the shared runtime. Must be called after
     * [Bridge.initialize] on the same process; throws (loudly) when it
     * isn't — the runtime cannot outlive its backend.
     */
    fun start(context: Context): TurRuntime {
        val backendHandle = bridge.getBackendId()
        if (backendHandle <= 0L) {
            val msg = "EaseBackend.start: backend not initialized " +
                "(backendId=$backendHandle) — call Bridge.initialize() first"
            Log.e(TAG, msg)
            bridge.logRaw("error", msg)
            throw IllegalStateException(msg)
        }
        synchronized(lock) {
            runtimeField?.let { existing ->
                if (existing.handle != 0L) {
                    val msg = "EaseBackend.start: already running " +
                        "(runtime=${existing.handle}) — returning existing"
                    Log.w(TAG, msg)
                    bridge.logRaw("error", msg)
                    return existing
                }
                // A closed runtime somehow lingered — clear it.
                val msg = "EaseBackend.start: dropping stale closed runtime"
                Log.w(TAG, msg)
                bridge.logRaw("error", msg)
                runtimeField = null
                _runtime.value = null
            }

            Log.i(TAG, "EaseBackend.start: creating worker pools")
            val pools = com.kutedev.easemusicplayer.turintegration.EasePluginBridge.createPluginWorkerPools()
            if (pools == 0L) {
                val msg = "EaseBackend.start: createPluginWorkerPools returned 0"
                Log.e(TAG, msg)
                bridge.logRaw("error", msg)
                throw IllegalStateException(msg)
            }

            Log.i(TAG, "EaseBackend.start: creating runtime (pools=$pools backend=$backendHandle)")
            val handle = com.kutedev.easemusicplayer.turintegration.EasePluginBridge.createRuntime(
                context.applicationContext,
                pools,
                backendHandle,
            )
            if (handle == 0L) {
                com.kutedev.easemusicplayer.turintegration.EasePluginBridge.destroyPluginWorkerPools(pools)
                val msg = "EaseBackend.start: createRuntime returned 0 " +
                    "(backend handle $backendHandle; see logcat)"
                Log.e(TAG, msg)
                bridge.logRaw("error", msg)
                throw IllegalStateException(msg)
            }

            Log.i(TAG, "EaseBackend.start: binding runtime $handle to backend $backendHandle")
            com.kutedev.easemusicplayer.turintegration.EasePluginBridge.bindPluginRuntime(
                backendHandle,
                handle,
                pools,
                context.applicationContext.assets,
            )

            val runtime = TurRuntime(handle, pools)
            runtimeField = runtime
            _runtime.value = runtime
            bridge.logRaw(
                "info",
                "backend started (runtime=$handle pools=$pools backend=$backendHandle)",
            )
            Log.i(TAG, "EaseBackend.start: runtime $handle up")
            return runtime
        }
    }

    /** The live runtime, or `null` when not started / stopped. */
    fun runtimeOrNull(): TurRuntime? = synchronized(lock) { runtimeField }

    /**
     * Unbind + destroy the runtime. Pass a human-readable [reason] — it is
     * logged on both channels. Idempotent; safe to call when never started.
     * MUST run before [Bridge.destroy] (the unbind needs a resolvable
     * backend handle).
     */
    fun stop(reason: String) {
        synchronized(lock) {
            val runtime = runtimeField
            if (runtime == null) {
                Log.i(TAG, "EaseBackend.stop('$reason'): nothing running")
                return
            }
            Log.i(TAG, "EaseBackend.stop('$reason'): unbinding + destroying runtime ${runtime.handle}")
            val backendHandle = bridge.getBackendId()
            if (backendHandle > 0L) {
                com.kutedev.easemusicplayer.turintegration.EasePluginBridge.unbindPluginRuntime(
                    backendHandle,
                    runtime.handle,
                )
            } else {
                val msg = "EaseBackend.stop: backend handle unavailable " +
                    "($backendHandle) — Rust-side runtime binding not cleared"
                Log.w(TAG, msg)
                bridge.logRaw("error", msg)
            }
            runtime.close()
            runtimeField = null
            _runtime.value = null
            bridge.logRaw("info", "backend stopped: $reason")
            Log.i(TAG, "EaseBackend.stop('$reason'): done")
        }
    }

    private companion object {
        private const val TAG = "EaseBackend"
    }
}
