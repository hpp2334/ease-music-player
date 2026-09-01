package com.kutedev.easemusicplayer.turintegration

/**
 * JNI bridge to the native tur engine standard operations.
 *
 * Every method is a thin `external fun` over the hand-written
 * `Java_com_kutedev_easemusicplayer_turintegration_TurNative_*` entry points
 * that `libease_client_backend.so` exports (see `rust-libs/ease-client-backend/
 * src/plugin_runtime/plugin_jni.rs`).
 *
 * The library is loaded once by `EaseMusicPlayerApplication`'s
 * `companion object { init { System.loadLibrary("ease_client_backend") } }`,
 * so this object doesn't need its own `System.loadLibrary` call — the
 * external-fn bindings resolve against the already-loaded library.
 *
 * Handle-based and stateless: the same object safely drives any number
 * of instances — one per [TurView], or a headless service instance — all
 * sharing one runtime built via `EasePluginBridge.createRuntime`.
 *
 * **Threading**: every op is marshalled onto the native **tur-host thread**
 * — the single thread that owns the runtime, instances, and renderers — so
 * these functions may be called from any thread (in practice: the main
 * looper, where surface + input callbacks arrive) and never run engine work
 * on the caller. Fire-and-forget ops return immediately; the op queue is
 * FIFO, so ordering is preserved. A native build failure inside a posted op
 * (e.g. wgpu init) surfaces as a logcat error and later ops for that handle
 * become no-ops.
 */
object TurNative {
    /**
     * Spawn an isolated **renderer-less** engine instance — the INITIALIZE
     * half of tur's two-phase lifecycle (#215). No surface yet: the native
     * build (worker handshake + plugin registration) is queued onto the
     * native tur-host thread while the caller stays free; ops for this
     * handle are ordered behind it. Attach a rendering surface later via
     * [attachInstance] (from `surfaceCreated`); a never-attached instance
     * is simply headless. [pluginId] is stamped into the instance's
     * per-instance data slot so `ease:*` bridge fns can resolve the
     * calling plugin without a JS argument. [instance] is the storage's
     * `plugin_storage_id` for edit-mode views (empty string for create
     * mode) — exposed to JS as `ease.context.instance()`. [poolsHandle]
     * (from [`EasePluginBridge.createPluginWorkerPools`]) assigns the
     * instance to the shared view worker pool; `0` keeps the engine
     * default. A build failure logs to logcat (no exception) and turns
     * later ops into no-ops. Returns `0` only for an invalid runtime
     * handle or if the host thread is gone.
     */
    external fun createInstance(
        runtimeHandle: Long,
        poolsHandle: Long,
        frameLoop: FrameLoop,
        pluginId: String,
        instance: String,
    ): Long

    /**
     * Spawn an isolated headless instance (never attached to a surface).
     * Runs JS + capabilities + events only. [pluginId] mirrors
     * [`createInstance`]'s identity stamp. [poolsHandle] (from
     * [`EasePluginBridge.createPluginWorkerPools`]) assigns the instance to
     * the shared backend worker pool; `0` keeps the engine default. Same
     * async-build semantics as [`createInstance`]. Returns an opaque instance
     * handle, or `0` on failure.
     */
    external fun createHeadlessInstance(
        runtimeHandle: Long,
        poolsHandle: Long,
        frameLoop: FrameLoop,
        pluginId: String,
    ): Long

    /**
     * Attach a rendering surface to a built instance — the ATTACH half of
     * the two-phase lifecycle (call from `surfaceCreated`, where the
     * `Surface` is guaranteed valid). The attach op is ordered behind the
     * instance build, so the instance exists when it runs: it acquires
     * the native window, performs the wgpu surface/adapter/device init,
     * and hands the renderer to the engine. If the instance is already
     * gone (destroyed / failed build) or the wgpu init fails, the error
     * is logged and the instance stays renderer-less — attachable again
     * later. Pair with [detachInstance] on `surfaceDestroyed`; the pair
     * is repeatable (a re-created surface re-attaches without rebuilding
     * the JS realm).
     */
    external fun attachInstance(
        handle: Long,
        surface: android.view.Surface,
        width: Int,
        height: Int,
        dpr: Double,
    )

    /**
     * Detach the rendering surface — the DETACH half (call from
     * `surfaceDestroyed`). Drops the renderer and releases the native
     * window reference; the instance keeps running (JS, capabilities,
     * events) and can attach a fresh surface later. Idempotent.
     */
    external fun detachInstance(handle: Long)

    /** Register a JS module source on the runtime's shared registry and
     *  return its opaque handle (`0` on failure). The source crosses JNI
     *  exactly once, here; [loadModule] then loads it into any instance of
     *  the runtime by handle — no per-load string copies. Sources created
     *  on the Rust side (the plugin scan's `plugin.list`) never cross at
     *  all — Kotlin only ever sees the opaque [Long]. */
    external fun registerModuleSource(runtimeHandle: Long, js: String): Long

    /** Drop a registered module source. Idempotent — a stale/unknown
     *  handle is a no-op. Everything left registered is freed when the
     *  runtime is destroyed. */
    external fun releaseModuleSource(runtimeHandle: Long, sourceHandle: Long)

    /**
     * Evaluate the registered module source ([registerModuleSource]'s
     *  return value, or a Rust-side `plugin.list` handle) as an ES module
     *  (`import … from "tur:*"` resolved by the engine) and request a
     *  paint. The shared source flows to the engine by refcount — zero
     *  JNI string traffic. Posted onto the native tur-host thread
     *  (ordered behind the instance build); a failed load logs to logcat.
     */
    external fun loadModule(handle: Long, sourceHandle: Long)

    /**
     * Fire one engine wake — call each Choreographer / Handler tick. Posts
     * the vsync pump onto the native tur-host thread (which fires the vsync
     * event, polls the loop, and applies the frame's render batch to the
     * GPU); the calling thread returns immediately.
     */
    external fun pump(handle: Long): Int

    /**
     * Poll the engine's loop WITHOUT firing a vsync — posted onto the
     * native tur-host thread (the coalesced message-pump path for
     * worker→host messages / host-loop tasks while the engine is idle).
     * Keeps an idle instance from ping-ponging at display refresh rate.
     */
    external fun pumpMessages(handle: Long): Int

    /** Push a new surface size (logical px + dpr). */
    external fun resize(handle: Long, width: Int, height: Int, dpr: Double)

    /**
     * Push a pointer event. [action] matches `MotionEvent.ACTION_*`:
     * `0=DOWN`, `1=UP`, `2=MOVE`, `3=CANCEL`. Coordinates are logical px
     * relative to the surface; [timeMs] is `SystemClock.uptimeMillis()`.
     */
    external fun pushPointer(handle: Long, action: Int, x: Double, y: Double, timeMs: Long)

    /**
     * Push a key event. [action] is `0=DOWN`, `1=UP`. [key]/[code] are
     * browser-style strings (see [InputMapper] for the Android→browser map).
     */
    external fun pushKey(
        handle: Long,
        key: String,
        code: String,
        action: Int,
        ctrl: Boolean,
        shift: Boolean,
        alt: Boolean,
        meta: Boolean,
    )

    /**
     * Push an IME composition event. [kind]: `0=CompositionStart`,
     * `1=CompositionUpdate { text }`, `2=CompositionEnd { text }`.
     */
    external fun pushIme(handle: Long, kind: Int, text: String)

    /** Drop the instance and free its resources (the runtime is unaffected). */
    external fun destroy(handle: Long)

    /**
     * Drop an instance and **block until its teardown settled** on the
     * native tur-host thread: the destroy op ran, and with it the
     * instance's renderer, surface, and loop future were dropped. Returns
     * `true` when settled, `false` if the host thread had already shut
     * down.
     *
     * The fence for hosts that must know disposal finished — e.g. before
     * re-creating an instance on the same surface — replacing sleep-based
     * quiesce heuristics. **Blocking** (it waits behind any ops still
     * queued for the instance, including an in-flight build): call off
     * the main thread, e.g. `withContext(Dispatchers.Default) {
     * TurNative.destroySettled(handle) }`.
     */
    external fun destroySettled(handle: Long): Boolean

    /** Drop the runtime and free its resources. Destroy all instances first. */
    external fun destroyRuntime(handle: Long)
}
