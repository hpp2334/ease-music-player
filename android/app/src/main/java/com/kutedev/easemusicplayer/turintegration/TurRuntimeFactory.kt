package com.kutedev.easemusicplayer.turintegration

import android.content.Context

/**
 * Builds a native tur runtime (the shared, created-once substrate) and returns
 * its opaque handle.
 *
 * Implemented as a thin wrapper around the app's own
 * `external fun createRuntime(...)` JNI function exported by
 * `libease_client_backend.so` (see
 * `Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_createRuntime`).
 *
 * The runtime is created **once** (system-font discovery + plugin registration
 * happen a single time); isolated [TurInstance]s are then spawned from it via
 * [TurRuntime.createInstance] / [TurRuntime.createHeadlessInstance].
 *
 * @return the opaque runtime handle (a `Long`), or `0` on failure.
 */
fun interface TurRuntimeFactory {
    fun create(context: Context): Long
}
