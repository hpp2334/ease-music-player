package com.kutedev.easemusicplayer.turintegration

import android.content.Context
import android.graphics.PixelFormat
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView

/**
 * A Compose surface that runs a tur engine instance and renders the given JS.
 *
 * Drop this composable into any Compose UI, pass a JS bundle string (an ES
 * module importing from `tur:std` / `ease:storage` / etc.) and a
 * [TurEngineFactory] (which builds the native engine and returns its opaque
 * handle). Pointer (touch) and resize are wired automatically.
 *
 * @param js an ES module source. Imports of `tur:*` / `ease:*` are resolved
 *   by the engine's module loader.
 * @param engineFactory builds the native engine over the surface and returns
 *   its handle. The app owns this — `libease_client_backend.so` is loaded
 *   by the `Application`'s companion object.
 * @param dpr force a DPR (defaults to the window's display density).
 */
@Composable
fun TurView(
    js: String,
    engineFactory: TurEngineFactory,
    modifier: Modifier = Modifier,
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
        surfaceView.bind(js, context, resolvedDpr, engineFactory)
        onDispose { surfaceView.unbind() }
    }
}

/**
 * `SurfaceView` subclass that owns the [TurEngine] lifecycle + touch dispatch.
 *
 * The engine is created lazily via [bind] once the surface is ready. All
 * methods must be called on the main looper.
 */
private class TurSurfaceView(context: Context) : SurfaceView(context) {
    private var engine: TurEngine? = null
    private var pendingJs: String? = null
    private var dprValue: Double = 0.0
    private var engineFactory: TurEngineFactory? = null

    init {
        // SurfaceView renders on its own layer; place it on top with an
        // RGBA_8888 format so the engine's clear color shows directly.
        setZOrderOnTop(true)
        holder.setFormat(PixelFormat.RGBA_8888)
    }

    fun bind(js: String, context: Context, dpr: Double, factory: TurEngineFactory) {
        pendingJs = js
        dprValue = dpr
        engineFactory = factory
        isFocusable = true
        isFocusableInTouchMode = true
        requestFocus()
        holder.addCallback(surfaceCallback)
        setOnTouchListener { _, event ->
            val eng = engine ?: return@setOnTouchListener false
            val d = dprValue.coerceAtLeast(1.0)
            eng.pushPointer(
                event.actionMasked,
                event.x.toDouble() / d,
                event.y.toDouble() / d,
                event.eventTime,
            )
            true
        }
    }

    fun unbind() {
        holder.removeCallback(surfaceCallback)
        setOnTouchListener(null)
        engine?.close()
        engine = null
    }

    private val surfaceCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (engine != null) return
            val js = pendingJs ?: return
            val factory = engineFactory ?: return
            // SurfaceHolder.surfaceFrame reports *physical* pixels; the
            // engine's viewport is in *logical* px, so divide by dpr.
            val d = dprValue.coerceAtLeast(1.0)
            val w = (holder.surfaceFrame.width() / d).toInt().coerceAtLeast(1)
            val h = (holder.surfaceFrame.height() / d).toInt().coerceAtLeast(1)
            engine = try {
                val frameLoop = FrameLoop()
                val handle = factory.create(context, holder.surface, w, h, dprValue, frameLoop)
                if (handle == 0L) {
                    android.util.Log.e("TurView", "engineFactory.create returned 0 (see logcat)")
                    null
                } else {
                    TurEngine(handle, frameLoop).also { it.loadModule(js) }
                }
            } catch (e: Throwable) {
                android.util.Log.e("TurView", "engine create failed", e)
                null
            }
        }

        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            val d = dprValue.coerceAtLeast(1.0)
            engine?.resize(
                (width / d).toInt().coerceAtLeast(1),
                (height / d).toInt().coerceAtLeast(1),
                dprValue,
            )
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {
            engine?.close()
            engine = null
        }
    }
}
