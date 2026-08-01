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
 * A Compose surface that spawns an isolated tur instance from [runtime] and
 * renders the given JS into it.
 *
 * Drop this composable into any Compose UI, pass the shared [TurRuntime] and
 * a JS bundle string (an ES module importing from `tur:std` / `ease:storage` /
 * etc.). When the surface becomes ready the view spawns an instance via
 * [TurRuntime.createInstance]; when the surface is destroyed the instance is
 * torn down (the runtime survives, shared across views). Pointer (touch) and
 * resize are wired automatically.
 *
 * @param runtime the shared [TurRuntime] to spawn the instance from.
 * @param js an ES module source. Imports of `tur:*` / `ease:*` are resolved
 *   by the engine's module loader.
 * @param dpr force a DPR (defaults to the window's display density).
 */
@Composable
fun TurView(
    runtime: TurRuntime,
    js: String,
    modifier: Modifier = Modifier,
    dpr: Double? = null,
) {
    val context = LocalContext.current
    val resolvedDpr = dpr ?: context.resources.displayMetrics.density.toDouble()

    val surfaceView = remember { TurSurfaceView(context, runtime, resolvedDpr) }

    AndroidView(
        factory = { surfaceView },
        modifier = modifier.fillMaxSize(),
    )

    DisposableEffect(surfaceView) {
        surfaceView.bind(js)
        onDispose { surfaceView.unbind() }
    }
}

/**
 * `SurfaceView` subclass that owns the [TurInstance] lifecycle + touch dispatch.
 *
 * The instance is created lazily via [bind] once the surface is ready, spawned
 * from the [TurRuntime] passed at construction. All methods must be called on
 * the main looper.
 */
private class TurSurfaceView(
    context: Context,
    private val runtime: TurRuntime,
    private val dprValue: Double,
) : SurfaceView(context) {
    private var instance: TurInstance? = null
    private var pendingJs: String? = null

    init {
        // SurfaceView renders on its own layer; place it on top with an
        // RGBA_8888 format so the engine's clear color shows directly.
        setZOrderOnTop(true)
        holder.setFormat(PixelFormat.RGBA_8888)
    }

    fun bind(js: String) {
        pendingJs = js
        isFocusable = true
        isFocusableInTouchMode = true
        requestFocus()
        holder.addCallback(surfaceCallback)
        setOnTouchListener { _, event ->
            val inst = instance ?: return@setOnTouchListener false
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

    fun unbind() {
        holder.removeCallback(surfaceCallback)
        setOnTouchListener(null)
        instance?.setAfterPump(null)
        instance?.close()
        instance = null
    }

    private val surfaceCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (instance != null) return
            val js = pendingJs ?: return
            // SurfaceHolder.surfaceFrame reports *physical* pixels; the
            // engine's viewport is in *logical* px, so divide by dpr.
            val d = dprValue.coerceAtLeast(1.0)
            val w = (holder.surfaceFrame.width() / d).toInt().coerceAtLeast(1)
            val h = (holder.surfaceFrame.height() / d).toInt().coerceAtLeast(1)
            instance = try {
                runtime.createInstance(holder.surface, w, h, dprValue).also {
                    it.loadModule(js)
                }
            } catch (e: Throwable) {
                android.util.Log.e("TurView", "instance create failed", e)
                null
            }
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
            instance?.close()
            instance = null
        }
    }
}
