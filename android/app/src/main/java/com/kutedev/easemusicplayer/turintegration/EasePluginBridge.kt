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
 * destroy) is owned explicitly by [PluginRuntimeHost], which logs every
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
     * bundled plugin zips natively (`plugin.bootstrap`). Only the handles
     * cross JNI — never the JS or zip bytes. Call once right after
     * [createRuntime], as part of [PluginRuntimeHost.start].
     */
    @JvmStatic
    external fun bindPluginRuntime(backendHandle: Long, runtimeHandle: Long, assetManager: android.content.res.AssetManager)

    /**
     * The teardown counterpart of [bindPluginRuntime]: clears the stored
     * runtime handle (compare-and-set — a stale stop can never clobber a
     * newer binding) and drops the stashed `AAssetManager`. Call BEFORE
     * `TurNative.destroyRuntime`, while [backendHandle] is still alive —
     * afterwards every `plugin.list` module-source handle would silently
     * come back 0.
     */
    @JvmStatic
    external fun unbindPluginRuntime(backendHandle: Long, runtimeHandle: Long)

    /**
     * Connect a headless backend instance's event bus to ease-tur-rpc and
     * stash the resulting `Send` `RpcClient` into the named backend
     * instance's context under [pluginId]. Call once per plugin, after
     * `createHeadlessInstance` + `loadModule` (by source handle) so the JS
     * dispatcher + backend handlers are registered — the native op queue is
     * FIFO, so the wire round-trip lands behind both even though the
     * instance build is async. Returns `true` on success.
     */
    @JvmStatic
    external fun wireServiceRpc(backendHandle: Long, instanceHandle: Long, pluginId: String): Boolean

    /**
     * Drop the backend context's service `RpcClient` entry for [pluginId]
     * (its headless instance is being torn down — the plugin was disabled /
     * uninstalled / upgraded). Storage dispatch + event delivery for the
     * plugin degrade gracefully until a fresh instance is wired.
     */
    @JvmStatic
    external fun unwireServiceRpc(backendHandle: Long, pluginId: String)
}
