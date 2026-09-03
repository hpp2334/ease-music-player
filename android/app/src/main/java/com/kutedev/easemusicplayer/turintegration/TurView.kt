package com.kutedev.easemusicplayer.turintegration

import android.content.Context
import android.graphics.Bitmap
import android.graphics.PixelFormat
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.PixelCopy
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.delay

/**
 * A Compose surface that spawns an isolated tur instance from [runtime] and
 * renders the given module source into it.
 *
 * Drop this composable into any Compose UI, pass the shared [TurRuntime],
 * a module **source handle** (an ES module source registered on the
 * runtime via [TurRuntime.registerModuleSource] or created on the Rust
 * side — the `plugin.list` scan), the `pluginId` this instance is being
 * created for, and (for edit views) the storage's `plugin_storage_id`.
 * The plugin id is stamped into the instance's per-instance data slot so
 * `ease:*` bridge fns resolve the calling plugin from Rust, not from a JS
 * argument; the instance id is exposed to JS as `ease.context.instance()`.
 *
 * Two-phase lifecycle (tur #215): the view spawns a renderer-less instance
 * via [TurRuntime.createInstance] as soon as it binds and loads the
 * module; `surfaceCreated` ATTACHES the surface ([TurInstance.attach])
 * and `surfaceDestroyed` DETACHES it ([TurInstance.detach]) — the
 * instance (JS realm + module state) survives surface recreation and
 * re-attaches; leaving the composition destroys it. Pointer (touch),
 * resize, soft keyboard (IME), and basic hardware-key dispatch are wired
 * automatically.
 *
 * While the instance builds and the module's first frame is pending, a
 * [loadingIndicator] (a small spinner by default) is shown in the window
 * layer — visible through the surface until its first buffer composites
 * (the z-order story is commented at the indicator's call site below).
 *
 * @param runtime the shared [TurRuntime] to spawn the instance from.
 * @param sourceHandle a registered module-source handle (from
 *   [TurRuntime.registerModuleSource] or the Rust-side `plugin.list`).
 *   The source is an ES module importing from `tur:std` / `ease`,
 *   resolved by the engine's module loader. Loading by handle means the
 *   bundle never crosses the Kotlin↔Rust boundary as a string.
 * @param pluginId the plugin this instance is being created for. Stamped
 *   into the per-instance data slot; bridge fns in `ease:*` read it via
 *   `extract_js_ctx` + `data::<PluginId>()`.
 * @param instance the storage's `plugin_storage_id` for edit views
 *   (`null` for create-mode setup views); exposed to JS as
 *   `ease.context.instance()`.
 * @param dpr force a DPR (defaults to the window's display density).
 * @param loadingIndicator rendered in place of the plugin UI until the
 *   surface composites its first buffer (instance build + module eval +
 *   mount + first render + GPU init — detected by polling the surface
 *   with `PixelCopy`, see `TurSurfaceView`). Pass `null` to disable.
 */
@Composable
fun TurView(
    runtime: TurRuntime,
    sourceHandle: Long,
    pluginId: String,
    modifier: Modifier = Modifier,
    instance: String? = null,
    dpr: Double? = null,
    loadingIndicator: (@Composable () -> Unit)? = { DefaultTurLoadingIndicator() },
) {
    val context = LocalContext.current
    val resolvedDpr = dpr ?: context.resources.displayMetrics.density.toDouble()

    // Loading → loaded: flipped (on the main looper) when the surface
    // composites its first buffer (PixelCopy-detected, see TurSurfaceView),
    // or by the timeout guard below. Plain `remember`: re-entering the
    // page re-creates the instance + surface, so the indicator re-shows.
    var ready by remember { mutableStateOf(false) }

    val surfaceView = remember { TurSurfaceView(context) }

    Box(modifier = modifier.fillMaxSize()) {
        // The tur surface is z-order-on-top — it composites ABOVE the whole
        // window — so this indicator can never draw over it. It works the
        // other way around: until the engine queues its first buffer, the
        // surface layer composites nothing, so this window-layer content
        // shows through; the plugin's first (opaque) frame then covers it,
        // and the PixelCopy signal removes it from composition within one
        // poll interval of that. Dismissal is signal-driven, not
        // transparency-driven — a plugin painting a transparent background
        // still loses the indicator (a beat late at worst, hidden behind
        // whatever it did paint).
        if (!ready) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.surface),
                contentAlignment = Alignment.Center,
            ) {
                loadingIndicator?.invoke()
            }
        }
        AndroidView(
            factory = { surfaceView },
            modifier = Modifier.fillMaxSize(),
        )
    }

    DisposableEffect(surfaceView) {
        surfaceView.bind(runtime, sourceHandle, pluginId, instance, resolvedDpr) { ready = true }
        onDispose { surfaceView.unbind() }
    }

    // Stuck-load guard: a failed instance build or module load is log-only
    // (see [TurSurfaceView.bind] and [TurNative.loadModule]) — without this
    // the indicator would spin forever on a load that never renders.
    LaunchedEffect(Unit) {
        delay(TUR_LOADING_TIMEOUT_MS)
        ready = true
    }
}

/** [TurView]'s stuck-load guard: drop the loading indicator after 10 s. */
private const val TUR_LOADING_TIMEOUT_MS = 10_000L

/** First-buffer poll cadence (`TurSurfaceView`'s PixelCopy loop). */
private const val FIRST_FRAME_POLL_MS = 100L

/** Probe size for the first-buffer PixelCopy — only the call's STATUS
 *  matters, never the pixels, so keep it tiny; PixelCopy scales the
 *  surface into whatever the destination bitmap is. */
private const val FIRST_FRAME_PROBE_SIZE = 16

/** Default [TurView] loading indicator — the app-standard small spinner
 *  (same size/stroke as the plugin-management busy rows). */
@Composable
private fun DefaultTurLoadingIndicator() {
    CircularProgressIndicator(
        modifier = Modifier.size(24.dp),
        strokeWidth = 2.dp,
    )
}

/**
 * `SurfaceView` subclass that owns the [TurInstance] lifecycle + input dispatch.
 *
 * Two-phase lifecycle (tur #215): the instance (JS realm, worker, plugins)
 * is created in [bind] — no surface involved, so nothing can race
 * Android's surface lifecycle — and `loadModule` is ordered right behind
 * the native build (FIFO). `surfaceCreated` ATTACHES the surface;
 * `surfaceDestroyed` DETACHES it (the instance survives and re-attaches
 * when the platform re-creates the surface); [unbind] destroys the
 * instance.
 *
 * All methods run on the main looper (where `SurfaceHolder.Callback` and
 * input dispatch arrive) — they only marshal work onto the native
 * tur-host thread, which runs the engine + GPU work off the main thread;
 * [createInstance][TurRuntime.createInstance] returns before the native
 * build finishes (failures log to logcat).
 */
private class TurSurfaceView(context: Context) : SurfaceView(context) {
    private var instance: TurInstance? = null
    private var dprValue: Double = 0.0
    /** Delivers PixelCopy's async completion on the main looper. */
    private val mainHandler = Handler(Looper.getMainLooper())
    /** Tracks the last IME state we drove so we only call the IMM on
     *  show↔hide transitions (not every frame). */
    private var imeActive = false
    /** True once the user has touched the surface. We suppress `showSoftInput`
     *  until then so a programmatically-focused editable (e.g. the editor
     *  auto-focusing on launch) doesn't pop the keyboard unprompted — the
     *  keyboard should only appear in response to a user tap. */
    private var userInteracted = false

    // --- Loading-indicator support ------------------------------------------
    //
    // The first frame's arrival is detected from the Android side, not the
    // engine's: a z-order-on-top SurfaceView composites NOTHING until its
    // first buffer is queued, and PixelCopy reports exactly that (an error
    // while the surface is still buffer-less, SUCCESS once the first buffer
    // landed). The engine-side alternatives are unusable from Kotlin:
    // Choreographer wakes start at instance build (a bootstrap arm fires
    // long before anything paints, so "a vsync happened" says nothing about
    // rendering), and the engine's true `FrameOutcome.painted` isn't
    // exposed over the pinned tur-android rev. Polling is deliberately
    // coarse (100 ms): a LATE dismissal is invisible — the first frame is
    // opaque and covers the indicator — while an early one re-opens the
    // blank gap this mechanism exists to cover.

    /** [bind]'s one-shot first-frame callback (drives TurView's indicator). */
    private var firstFrameCallback: (() -> Unit)? = null
    /** One-shot latch for [firstFrameCallback]. */
    private var firstFrameFired = false
    /** 16×16 mutable probe reused by every [pollFirstFrame] round. */
    private var probe: Bitmap? = null
    /** Guard against overlapping async PixelCopy rounds. */
    private var pollInFlight = false
    /** Whether polling is armed (surface exists + callback + not yet fired). */
    private var polling = false
    /** Single reusable self-post (postDelayed/removeCallbacks pair on it). */
    private val pollRunnable = Runnable { pollFirstFrame() }

    init {
        // SurfaceView renders on its own layer below the view hierarchy by
        // default; the Compose host window's opaque background would cover it.
        // Put this surface on top so the tur-rendered content is visible, and
        // use RGBA_8888 so the engine's clear color shows directly.
        setZOrderOnTop(true)
        holder.setFormat(PixelFormat.RGBA_8888)
    }

    /** Stash the dpr, spawn the renderer-less instance (stamped with
     *  [pluginId]/[instanceId]), load the module, and register the surface
     *  callback; attach the surface when it's ready. [onFirstFrame] fires
     *  (once, on the main looper) when the surface composites its first
     *  buffer — used by [TurView] to drop its loading indicator. */
    fun bind(
        runtime: TurRuntime,
        sourceHandle: Long,
        pluginId: String,
        instanceId: String?,
        dpr: Double,
        onFirstFrame: (() -> Unit)? = null,
    ) {
        dprValue = dpr
        firstFrameCallback = onFirstFrame
        firstFrameFired = false
        isFocusable = true
        isFocusableInTouchMode = true
        requestFocus()
        val t0 = android.os.SystemClock.elapsedRealtime()
        instance = try {
            // INITIALIZE — returns as soon as the handle exists; the build
            // (worker handshake + plugin registration) runs on the native
            // tur-host thread, and the loadModule below is queued behind
            // it (FIFO). The TurPerf timings therefore cover queueing, not
            // the build; `markBoot`→`markFirstPump` spans the whole async
            // build + module eval + first render.
            runtime.createInstance(pluginId, instanceId).also {
                val t1 = android.os.SystemClock.elapsedRealtime()
                if (sourceHandle != 0L) it.loadModule(sourceHandle)
                val t2 = android.os.SystemClock.elapsedRealtime()
                android.util.Log.d(
                    "TurPerf",
                    "bind: createInstance=${t1 - t0}ms loadModule=${t2 - t1}ms",
                )
                it.markBoot()
                // After each frame, sync the soft keyboard with the
                // engine's text-input state (reads the value native
                // pushed into the FrameLoop via onTextInputChanged).
                it.setAfterPump {
                    it.markFirstPump()
                    syncIme()
                }
            }
        } catch (e: Throwable) {
            android.util.Log.e("TurView", "instance create failed", e)
            null
        }
        holder.addCallback(surfaceCallback)
        setOnTouchListener { _, event ->
            userInteracted = true
            // Reconcile the IME on the user-interaction transition itself:
            // a tap on an already-focused editable may change no focus and
            // request no new frame, so nothing else would run the sync —
            // but the keyboard should appear now that the user has
            // interacted.
            syncIme()
            val inst = instance ?: return@setOnTouchListener false
            // `MotionEvent.getX/Y` are in physical px; the engine hit-tests in
            // logical px, so divide by dpr.
            val d = dprValue.coerceAtLeast(1.0)
            inst.pushPointer(
                event.actionMasked,
                event.x.toDouble() / d,
                event.y.toDouble() / d,
                event.eventTime,
            )
            true
        }
    }

    /** Tear down: remove callbacks + destroy the instance (runtime survives). */
    fun unbind() {
        stopFirstFramePoll()
        removeCallbacks(pollRunnable)
        holder.removeCallback(surfaceCallback)
        setOnTouchListener(null)
        instance?.setAfterPump(null)
        instance?.close()
        instance = null
        imeActive = false
        // Drop (don't recycle) the probe: an async PixelCopy round may still
        // be writing into it natively; GC handles a 16×16 bitmap just fine.
        probe = null
    }

    private val surfaceCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            val inst = instance ?: return
            // `SurfaceHolder.surfaceFrame` reports *physical* pixels; the
            // engine's viewport is in *logical* px, so divide by dpr.
            val d = dprValue.coerceAtLeast(1.0)
            val w = (holder.surfaceFrame.width() / d).toInt().coerceAtLeast(1)
            val h = (holder.surfaceFrame.height() / d).toInt().coerceAtLeast(1)
            // ATTACH: the native attach op is FIFO behind the instance
            // build, so the instance exists when it runs; the wgpu
            // surface/adapter/device init happens there.
            inst.attach(holder.surface, w, h, dprValue)
            // The surface exists (buffer-less) — start watching for its
            // first composited buffer (see the loading-indicator note).
            startFirstFramePoll()
        }

        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            val d = dprValue.coerceAtLeast(1.0)
            instance?.resize(
                (width / d).toInt().coerceAtLeast(1),
                (height / d).toInt().coerceAtLeast(1),
                dprValue,
            )
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {
            // DETACH — not destroy: the instance (JS realm, module state)
            // survives the platform surface going away and re-attaches
            // when the surface is created again.
            stopFirstFramePoll()
            instance?.detach()
        }
    }

    // --- First-buffer polling (loading indicator) ---------------------------

    /** Arm the PixelCopy poll — from `surfaceCreated`, where the (still
     *  buffer-less) surface exists. No-op when already fired or when [bind]
     *  got no [onFirstFrame] callback (indicator disabled). */
    private fun startFirstFramePoll() {
        if (polling || firstFrameFired || firstFrameCallback == null) return
        polling = true
        pollFirstFrame()
    }

    /** Disarm the poll — `surfaceDestroyed` / [unbind]. Re-armed by the next
     *  `surfaceCreated` if the first frame hasn't landed yet. */
    private fun stopFirstFramePoll() {
        polling = false
    }

    /**
     * One poll round: an async [PixelCopy] of this view into the tiny probe
     * bitmap. `SUCCESS` means the surface has queued its first buffer —
     * the plugin's first frame composited — so fire [firstFrameCallback]
     * once and stop. Any error (a buffer-less surface reports
     * `ERROR_SOURCE_NO_DATA`; a mid-teardown one other errors) just
     * schedules the next round while polling. The composable's timeout is
     * the backstop if no frame ever lands.
     */
    private fun pollFirstFrame() {
        if (!polling || firstFrameFired) return
        if (pollInFlight) return
        if (windowToken == null || !holder.surface.isValid) {
            postDelayed(pollRunnable, FIRST_FRAME_POLL_MS)
            return
        }
        val bmp = probe
            ?: Bitmap.createBitmap(FIRST_FRAME_PROBE_SIZE, FIRST_FRAME_PROBE_SIZE, Bitmap.Config.ARGB_8888)
                .also { probe = it }
        pollInFlight = true
        try {
            PixelCopy.request(this, bmp, { result ->
                pollInFlight = false
                if (result == PixelCopy.SUCCESS && !firstFrameFired) {
                    firstFrameFired = true
                    polling = false
                    firstFrameCallback?.invoke()
                } else if (polling && !firstFrameFired) {
                    postDelayed(pollRunnable, FIRST_FRAME_POLL_MS)
                }
            }, mainHandler)
        } catch (e: Throwable) {
            // Thrown synchronously (view detached / not yet attached to a
            // window, dead surface, …) — retry while polling; the timeout
            // stops the world if it never lands.
            pollInFlight = false
            if (polling && !firstFrameFired) postDelayed(pollRunnable, FIRST_FRAME_POLL_MS)
        }
    }

    override fun onKeyDown(keyCode: Int, event: android.view.KeyEvent): Boolean {
        val inst = instance ?: return super.onKeyDown(keyCode, event)
        val mapped = InputMapper.map(keyCode) ?: return super.onKeyDown(keyCode, event)
        inst.pushKey(
            key = mapped.first,
            code = mapped.second,
            action = 0, // DOWN
            ctrl = event.isCtrlPressed,
            shift = event.isShiftPressed,
            alt = event.isAltPressed,
            meta = event.isMetaPressed,
        )
        return true
    }

    override fun onKeyUp(keyCode: Int, event: android.view.KeyEvent): Boolean {
        val inst = instance ?: return super.onKeyUp(keyCode, event)
        val mapped = InputMapper.map(keyCode) ?: return super.onKeyUp(keyCode, event)
        inst.pushKey(
            key = mapped.first,
            code = mapped.second,
            action = 1, // UP
            ctrl = event.isCtrlPressed,
            shift = event.isShiftPressed,
            alt = event.isAltPressed,
            meta = event.isMetaPressed,
        )
        return true
    }

    // --- Soft keyboard / IME ------------------------------------------------
    //
    // The engine renders its own caret, so focus + the visible cursor work
    // without any platform IME. The missing piece is raising the soft
    // keyboard and routing its text back. We declare the surface a text editor
    // and supply a minimal `InputConnection` that turns IME commits into
    // engine events. The engine pushes its text-input state into the
    // FrameLoop (via `onTextInputChanged`); the per-frame `syncIme` reads that
    // retained value and drives `showSoftInput` / `hideSoftInput`.

    override fun onCheckIsTextEditor(): Boolean = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        val inst = instance ?: return null
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT
        // Avoid the fullscreen extract pane (phones, landscape) — the real
        // editor is our canvas; the extract UI would diverge from it.
        outAttrs.imeOptions =
            EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        return object : BaseInputConnection(this, false) {
            override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
                val s = text.toString()
                if (s.isEmpty()) return super.commitText(s, newCursorPosition)
                if (s.length == 1 && s[0].code < 128 && !s[0].isISOControl()) {
                    // Single ASCII printable char → key-event path (matches
                    // direct keyboard typing; the engine inserts `key` on
                    // keydown). DOWN then UP.
                    inst.pushKey(s, "", 0, false, false, false, false)
                    inst.pushKey(s, "", 1, false, false, false, false)
                } else {
                    // Multi-char / non-ASCII → composition insert (paste,
                    // autocorrect, CJK direct-commit).
                    inst.pushIme(0, "")
                    inst.pushIme(2, s)
                }
                return true
            }

            override fun deleteSurroundingText(
                beforeChars: Int,
                afterChars: Int,
            ): Boolean {
                // Backspace → existing key path (engine deletes on "Backspace").
                inst.pushKey("Backspace", "Backspace", 0, false, false, false, false)
                inst.pushKey("Backspace", "Backspace", 1, false, false, false, false)
                return true
            }
        }
    }

    /**
     * Raise/lower the soft keyboard to match the engine's text-input state.
     * The editable-focused flag is pushed from native into the
     * [FrameLoop] ([onTextInputChanged]); this reads the retained value and
     * reconciles the IMM. State-gated so the IMM is only touched on
     * show↔hide transitions, not every frame. Suppressed until the user has
     * actually touched the surface ([userInteracted]) so a launch-time
     * programmatic focus doesn't pop the keyboard unprompted.
     */
    private fun syncIme() {
        val inst = instance ?: return
        val imm = context
            .getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        if (inst.textInputActive() && userInteracted) {
            if (!hasFocus()) requestFocus()
            if (!imeActive) {
                imm.showSoftInput(this, 0)
                imeActive = true
            }
        } else if (imeActive) {
            imm.hideSoftInputFromWindow(windowToken, 0)
            imeActive = false
        }
    }
}
