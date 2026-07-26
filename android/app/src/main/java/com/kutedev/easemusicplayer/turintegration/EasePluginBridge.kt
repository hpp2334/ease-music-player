package com.kutedev.easemusicplayer.turintegration

import android.content.Context
import android.view.Surface

/**
 * JNI bridge to the ease-specific tur engine **creation** entry point.
 *
 * Mirrors `Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createEngine`
 * in `rust-libs/ease-client-backend/src/plugin_runtime/plugin_jni.rs`. The
 * standard engine-operation symbols (`Java_org_tur_TurNative_*`) live in
 * the same `.so` and are addressed via [TurNative].
 *
 * The library is already loaded by `EaseMusicPlayerApplication`'s
 * `companion object { init { System.loadLibrary("ease_client_backend") } }`,
 * so the `external fun` resolves at first call without an explicit
 * `System.loadLibrary` here.
 *
 * Call [createEngine] from inside a [TurEngineFactory] to obtain an
 * engine handle.
 */
object EasePluginBridge {
    /**
     * Build the tur engine over the given Android [Surface] with the Ease
     * plugin set (TurStdPlugin + TurAnimationPlugin + TurClipboardPlugin +
     * TurNetPlugin + EaseMusicPlugin) and return an opaque native handle.
     * Returns `0L` on failure (a `RuntimeException` is also thrown by the
     * native side).
     */
    @JvmStatic
    external fun createEngine(
        context: Context,
        surface: Surface,
        width: Int,
        height: Int,
        dpr: Double,
        frameLoop: FrameLoop,
    ): Long

    /** Convenience factory suitable for passing to [TurView]'s `engineFactory`. */
    val factory: TurEngineFactory = TurEngineFactory { ctx, surface, w, h, dpr, loop ->
        createEngine(ctx, surface, w, h, dpr, loop)
    }
}
