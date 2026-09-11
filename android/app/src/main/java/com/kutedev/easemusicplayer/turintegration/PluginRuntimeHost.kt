package com.kutedev.easemusicplayer.turintegration

import android.content.Context
import android.util.Log
import com.kutedev.easemusicplayer.singleton.Bridge
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Explicit lifecycle owner for the shared tur runtime + plugin worker pools.
 *
 * Replaces the old `EasePluginBridge.runtime(context)` get-cached-or-create
 * singleton, whose cache was never invalidated: when
 * [com.kutedev.easemusicplayer.core.KeepBackendService] was destroyed and
 * recreated inside one process, the cached (already-destroyed) runtime was
 * handed back out, `bindPluginRuntime` never re-ran, and every
 * `plugin.list` module-source handle silently came back 0 — plugin views and
 * backends never loaded again until the process died.
 *
 * The lifecycle is driven by exactly one owner (the service):
 *
 * 1. [start] — requires `Bridge.initialize()` to have run (the runtime binds
 *    to that backend instance's handle): pools → runtime →
 *    [EasePluginBridge.bindPluginRuntime]. Every step is logged (logcat +
 *    the in-app backend log). Idempotent: a second [start] logs and returns
 *    the live runtime instead of silently rebuilding (or reusing a corpse).
 * 2. [runtimeOrNull] / [runtime] — what UI pages consume; `null` means "not
 *    up (yet)" and callers render their fallback. The [StateFlow] makes
 *    pages self-heal when the service (re)starts.
 * 3. [stop] — [EasePluginBridge.unbindPluginRuntime] (while the backend
 *    handle is still resolvable!) → [TurRuntime.close]. Idempotent, reason
 *    logged.
 */
@Singleton
class PluginRuntimeHost @Inject constructor(
    private val bridge: Bridge,
) {
    private val lock = Any()

    private var runtimeField: TurRuntime? = null

    /** `null` = not started (or stopped — see the log for the reason). */
    private val _runtime = MutableStateFlow<TurRuntime?>(null)

    /** Observability for UI pages: goes non-null on [start], null on [stop]. */
    val runtime: StateFlow<TurRuntime?> = _runtime.asStateFlow()

    /**
     * Create + bind the shared runtime. Must be called after
     * [Bridge.initialize] on the same process; throws (loudly) when it
     * isn't — the runtime cannot outlive its backend.
     */
    fun start(context: Context): TurRuntime {
        val backendHandle = bridge.getBackendId()
        if (backendHandle <= 0L) {
            val msg = "PluginRuntimeHost.start: backend not initialized " +
                "(backendId=$backendHandle) — call Bridge.initialize() first"
            Log.e(TAG, msg)
            bridge.logRaw("error", msg)
            throw IllegalStateException(msg)
        }
        synchronized(lock) {
            runtimeField?.let { existing ->
                if (existing.handle != 0L) {
                    val msg = "PluginRuntimeHost.start: already running " +
                        "(runtime=${existing.handle}) — returning existing"
                    Log.w(TAG, msg)
                    bridge.logRaw("error", msg)
                    return existing
                }
                // A closed TurView-era instance somehow lingered — clear it.
                val msg = "PluginRuntimeHost.start: dropping stale closed runtime"
                Log.w(TAG, msg)
                bridge.logRaw("error", msg)
                runtimeField = null
                _runtime.value = null
            }

            Log.i(TAG, "PluginRuntimeHost.start: creating worker pools")
            val pools = EasePluginBridge.createPluginWorkerPools()
            if (pools == 0L) {
                val msg = "PluginRuntimeHost.start: createPluginWorkerPools returned 0"
                Log.e(TAG, msg)
                bridge.logRaw("error", msg)
                throw IllegalStateException(msg)
            }

            Log.i(TAG, "PluginRuntimeHost.start: creating runtime (pools=$pools backend=$backendHandle)")
            val handle = EasePluginBridge.createRuntime(
                context.applicationContext,
                pools,
                backendHandle,
            )
            if (handle == 0L) {
                EasePluginBridge.destroyPluginWorkerPools(pools)
                val msg = "PluginRuntimeHost.start: createRuntime returned 0 " +
                    "(backend handle $backendHandle; see logcat)"
                Log.e(TAG, msg)
                bridge.logRaw("error", msg)
                throw IllegalStateException(msg)
            }

            Log.i(TAG, "PluginRuntimeHost.start: binding runtime $handle to backend $backendHandle")
            EasePluginBridge.bindPluginRuntime(
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
                "plugin runtime started (runtime=$handle pools=$pools backend=$backendHandle)",
            )
            Log.i(TAG, "PluginRuntimeHost.start: runtime $handle up")
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
                Log.i(TAG, "PluginRuntimeHost.stop('$reason'): nothing running")
                return
            }
            Log.i(TAG, "PluginRuntimeHost.stop('$reason'): unbinding + destroying runtime ${runtime.handle}")
            val backendHandle = bridge.getBackendId()
            if (backendHandle > 0L) {
                EasePluginBridge.unbindPluginRuntime(backendHandle, runtime.handle)
            } else {
                val msg = "PluginRuntimeHost.stop: backend handle unavailable " +
                    "($backendHandle) — Rust-side runtime binding not cleared"
                Log.w(TAG, msg)
                bridge.logRaw("error", msg)
            }
            runtime.close()
            runtimeField = null
            _runtime.value = null
            bridge.logRaw("info", "plugin runtime stopped: $reason")
            Log.i(TAG, "PluginRuntimeHost.stop('$reason'): done")
        }
    }

    private companion object {
        private const val TAG = "EasePluginHost"
    }
}
