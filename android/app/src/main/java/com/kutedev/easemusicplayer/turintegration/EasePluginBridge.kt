package com.kutedev.easemusicplayer.turintegration

import android.content.Context

/**
 * JNI bridge to the ease-specific tur **runtime** creation entry points.
 *
 * Mirrors `Java_com_kutedev_easemusicplayer_turintegration_EasePluginBridge_*`
 * in `rust-libs/ease-client-android/src/plugin_runtime/plugin_jni.rs`. The
 * standard instance-operation symbols (`TurNative.*`) live in the same `.so`.
 *
 * The library is already loaded by `EaseMusicPlayerApplication`'s
 * `companion object { init { System.loadLibrary("ease_client_android") } }`,
 * so the `external fun` resolves at first call without an explicit
 * `System.loadLibrary` here.
 *
 * This object is a **stateless JNI surface only** — no cached runtime, no
 * get-or-create. The runtime lifecycle (create → bind → instances → unbind →
 * destroy) is owned explicitly by the `EaseBackend` facade, which logs every
 * transition; the backend a runtime binds to is named by the backend handle
 * passed into [createRuntime] / [bindPluginRuntime] (the same handle the
 * JSON bridge uses — there is no process-wide "current backend" singleton on
 * the Rust side anymore).
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
     * runtime; `0L` falls back to the engine default (one lane per instance).
     *
     * [backendHandle] binds the engine (and every instance spawned from it)
     * to one backend instance — the `ease:*` bridge fns resolve their
     * DB/KV/RPC services through it. An unknown handle fails the call
     * (returns `0L` + a Rust-side error log). Returns `0L` on failure (the
     * native side also throws).
     */
    @JvmStatic
    external fun createRuntime(context: Context, poolsHandle: Long, backendHandle: Long): Long

    /**
     * Hand the (already-created) tur runtime handle + the app `AssetManager`
     * to the named backend instance, so the Rust-side plugin manager can
     * register module sources on the runtime (`plugin.list`) and read
     * bundled plugin zips natively (`plugin.bootstrap`). `poolsHandle`
     * lets the backend's own headless-instance spawns assign the shared
     * `ease-plugin-backend` worker pool. Only the handles cross JNI — never
     * the JS or zip bytes. Call once right after [createRuntime], as part
     * of `EaseBackend.start`; attaching also triggers the first
     * Rust-side backend reload (scan + spawn + wire).
     */
    @JvmStatic
    external fun bindPluginRuntime(
        backendHandle: Long,
        runtimeHandle: Long,
        poolsHandle: Long,
        assetManager: android.content.res.AssetManager,
    )

    /**
     * The teardown counterpart of [bindPluginRuntime]: compare-and-set
     * detaches the engine binding (a stale stop can never clobber a newer
     * one) and tears down every Rust-owned headless backend instance +
     * its service RPC entry. Call BEFORE `TurNative.destroyRuntime`, while
     * [backendHandle] is still alive — afterwards every `plugin.list`
     * module-source handle would silently come back 0.
     */
    @JvmStatic
    external fun unbindPluginRuntime(backendHandle: Long, runtimeHandle: Long)
}
