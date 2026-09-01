package com.kutedev.easemusicplayer.turintegration

import android.content.Context
import android.graphics.PixelFormat
import android.text.InputType
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView

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
 */
@Composable
fun TurView(
    runtime: TurRuntime,
    sourceHandle: Long,
    pluginId: String,
    modifier: Modifier = Modifier,
    instance: String? = null,
    dpr: Double? = null,
) {
    val context = LocalContext.current
    val resolvedDpr = dpr ?: context.resources.displayMetrics.density.toDouble()

    val surfaceView = remember { TurSurfaceView(context) }

    AndroidView(
        factory = { surfaceView },
        modifier = modifier.fillMaxSize(),
    )

    DisposableEffect(surfaceView) {
        surfaceView.bind(runtime, sourceHandle, pluginId, instance, resolvedDpr)
        onDispose { surfaceView.unbind() }
    }
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
    /** Tracks the last IME state we drove so we only call the IMM on
     *  show↔hide transitions (not every frame). */
    private var imeActive = false
    /** True once the user has touched the surface. We suppress `showSoftInput`
     *  until then so a programmatically-focused editable (e.g. the editor
     *  auto-focusing on launch) doesn't pop the keyboard unprompted — the
     *  keyboard should only appear in response to a user tap. */
    private var userInteracted = false

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
     *  callback; attach the surface when it's ready. */
    fun bind(runtime: TurRuntime, sourceHandle: Long, pluginId: String, instanceId: String?, dpr: Double) {
        dprValue = dpr
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
        holder.removeCallback(surfaceCallback)
        setOnTouchListener(null)
        instance?.setAfterPump(null)
        instance?.close()
        instance = null
        imeActive = false
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
            instance?.detach()
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
