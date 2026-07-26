package com.kutedev.easemusicplayer.turintegration

/**
 * Builds a native tur engine over an Android [Surface] and returns its
 * opaque handle.
 *
 * Implemented as a thin wrapper around the app's own
 * `external fun createEngine(...)` JNI function exported by
 * `libease_client_backend.so` (see
 * `Java_com_kutedev_easemusicplayer_EasePluginBridge_createEngine`).
 *
 * The [FrameLoop] is created by [TurView] and passed in so the engine's
 * native loop driver can arm wake-ups against it.
 *
 * @return the opaque engine handle (a `Long`), or `0` on failure.
 */
fun interface TurEngineFactory {
    fun create(
        context: android.content.Context,
        surface: android.view.Surface,
        width: Int,
        height: Int,
        dpr: Double,
        frameLoop: FrameLoop,
    ): Long
}
