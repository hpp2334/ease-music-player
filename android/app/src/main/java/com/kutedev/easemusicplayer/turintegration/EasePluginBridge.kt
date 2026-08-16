package com.kutedev.easemusicplayer.turintegration

import android.content.Context

/**
 * JNI bridge to the ease-specific tur **runtime** creation entry point.
 *
 * Mirrors `Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createRuntime`
 * in `rust-libs/ease-client-backend/src/plugin_runtime/plugin_jni.rs`. The
 * standard instance-operation symbols (`TurNative.*`) live in the same `.so`.
 *
 * The library is already loaded by `EaseMusicPlayerApplication`'s
 * `companion object { init { System.loadLibrary("ease_client_backend") } }`,
 * so the `external fun` resolves at first call without an explicit
 * `System.loadLibrary` here.
 *
 * The runtime is built **once** (system-font discovery + plugin registration
 * happen a single time); [runtime] caches it for the app lifetime and hands
 * it to [TurView], which spawns an isolated instance per surface.
 */
object EasePluginBridge {
    /**
     * Allocate the two shared plugin worker pools (`ease-plugin-backend` /
     * `ease-plugin-view`, 2 lane threads each — all headless backends share
     * the former, all TurViews the latter) and return an opaque native
     * handle. Pass it to [createRuntime] (register on the runtime) and to
     * `TurNative.createInstance` / `createHeadlessInstance` (assign per
     * instance). Free with [destroyPluginWorkerPools] after
     * `TurNative.destroyRuntime`. Returns `0L` on failure.
     */
    @JvmStatic
    external fun createPluginWorkerPools(): Long

    /**
     * Free the worker pools behind [poolsHandle]. Call after the runtime
     * built with it is destroyed. `0L` is a no-op.
     */
    @JvmStatic
    external fun destroyPluginWorkerPools(poolsHandle: Long)

    /**
     * Build the shared tur runtime with the Ease plugin set
     * (TurStdPlugin + TurAnimationPlugin + TurClipboardPlugin + TurNetPlugin +
     * EaseMusicPlugin) and return its opaque native handle. A non-zero
     * [poolsHandle] also registers the shared plugin worker pools on the
     * runtime; `0L` falls back to the engine default (one lane thread per
     * instance). Returns `0L` on failure (the native side also throws).
     */
    @JvmStatic
    external fun createRuntime(context: Context, poolsHandle: Long): Long

    /**
     * Connect a headless backend instance's event bus to ease-tur-rpc and
     * stash the resulting `Send` `RpcClient` into the global backend context
     * under [pluginId]. Call once per plugin, on the instance's own (JNI)
     * thread, after `createHeadlessInstance` + `loadModule(backend.js)` so
     * the JS dispatcher + backend handlers are registered. Returns `true` on
     * success.
     */
    @JvmStatic
    external fun wireServiceRpc(instanceHandle: Long, pluginId: String): Boolean

    private var cached: TurRuntime? = null

    /**
     * The app-lifetime [TurRuntime], created lazily on first call (using the
     * application context so it is configuration-stable). Subsequent calls
     * return the same instance.
     */
    @Synchronized
    fun runtime(context: Context): TurRuntime {
        cached?.let { return it }
        val pools = createPluginWorkerPools()
        check(pools != 0L) { "createPluginWorkerPools returned 0 (see logcat)" }
        val handle = createRuntime(context.applicationContext, pools)
        check(handle != 0L) { "createRuntime returned 0 (see logcat)" }
        return TurRuntime(handle, pools).also { cached = it }
    }
}
