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
     * Build the shared tur runtime with the Ease plugin set
     * (TurStdPlugin + TurAnimationPlugin + TurClipboardPlugin + TurNetPlugin +
     * EaseMusicPlugin) and return its opaque native handle.
     * Returns `0L` on failure (the native side also throws).
     */
    @JvmStatic
    external fun createRuntime(context: Context): Long

    private var cached: TurRuntime? = null

    /**
     * The app-lifetime [TurRuntime], created lazily on first call (using the
     * application context so it is configuration-stable). Subsequent calls
     * return the same instance.
     */
    @Synchronized
    fun runtime(context: Context): TurRuntime {
        cached?.let { return it }
        val handle = createRuntime(context.applicationContext)
        check(handle != 0L) { "createRuntime returned 0 (see logcat)" }
        return TurRuntime(handle).also { cached = it }
    }
}
