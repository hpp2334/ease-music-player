package com.kutedev.easemusicplayer.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PluginManager
import com.kutedev.easemusicplayer.turintegration.PluginRuntimeHost
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import javax.inject.Inject


@AndroidEntryPoint
class KeepBackendService : Service() {
    @Inject lateinit var bridge: Bridge
    @Inject lateinit var pluginManager: PluginManager
    @Inject lateinit var pluginRuntimeHost: PluginRuntimeHost
    private val _channelId: String = "EaseMusicBackendServiceChannel"

    /** Runs the first-run bootstrap (bundled installs) off the main thread. */
    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var bootstrapped = false

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification = NotificationCompat.Builder(this, _channelId)
            .setContentTitle("Ease Music Player Backend")
            .setContentText("Ease Music Player Backend Service is running")
            .setOngoing(true)
            .build();

        startForeground(1, notification)

        bridge.initialize()
        bridge.logRaw("info", "KeepBackendService started")
        bootstrapServicePlugin()
        return START_NOT_STICKY
    }

    /**
     * Bring up the shared plugin runtime and kick the first-run install
     * bootstrap. That is ALL this service does now: since the backend owns
     * the headless-instance lifecycle (`reload_backends` in Rust, triggered
     * by the engine binding + every set-changing mutation), there is no
     * instance list, no revision collection, and no per-plugin wiring on
     * the Kotlin side anymore. The UI refreshes its plugin lists from the
     * `PLUGINS_CHANGED` signal (`EaseSignalHost`).
     */
    private fun bootstrapServicePlugin() {
        try {
            pluginRuntimeHost.start(this)
            if (!bootstrapped) {
                bootstrapped = true
                serviceScope.launch {
                    runCatching { pluginManager.bootstrapDefaults() }
                        .onFailure {
                            bridge.logRaw("error", "plugin bootstrap failed: ${it.message}")
                            Log.e(TAG, "plugin bootstrap failed", it)
                        }
                }
            }
        } catch (e: Throwable) {
            bridge.logRaw("error", "service plugin bootstrap failed: ${e.message}")
            Log.e(TAG, "service plugin bootstrap failed", e)
        }
    }

    override fun onBind(p0: Intent?): IBinder? {
        return null
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
    }

    override fun onDestroy() {
        super.onDestroy()
        serviceScope.cancel()
        // Unbind + destroy the runtime while the backend handle is still
        // resolvable — AFTER bridge.destroy() the Rust-side binding could
        // not be cleared anymore. The unbind itself tears down the
        // Rust-owned headless backends.
        pluginRuntimeHost.stop("KeepBackendService destroyed")
        bridge.destroy()
    }

    private fun createNotificationChannel() {
        val serviceChannel = NotificationChannel(
            _channelId,
            "Foreground Service Channel",
            NotificationManager.IMPORTANCE_LOW
        )

        val manager = getSystemService(
            NotificationManager::class.java
        )
        manager.createNotificationChannel(serviceChannel)
    }

    private companion object {
        private const val TAG = "KeepBackendService"
    }
}
