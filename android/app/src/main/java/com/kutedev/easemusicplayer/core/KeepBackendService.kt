package com.kutedev.easemusicplayer.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurInstance
import com.kutedev.easemusicplayer.turintegration.TurRuntime
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject


@AndroidEntryPoint
class KeepBackendService : Service() {
    @Inject lateinit var bridge: Bridge
    private val _channelId: String = "EaseMusicBackendServiceChannel"

    /** Held for the service lifetime so the headless instance is not GC'd. */
    private var serviceRuntime: TurRuntime? = null
    private var serviceInstance: TurInstance? = null

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
     * Bring up the headless tur instance that hosts JS service plugins
     * (currently the OneDrive storage provider). Sequence: build the shared
     * runtime (registers `TurRpcPlugin` + `EaseMusicPlugin`), spawn a headless
     * instance stamped with the loaded plugin's id, load the OneDrive plugin
     * bundle, then wire the event bus into a `Send` `RpcClient` the backend
     * can call from any thread. All steps run on the main looper (where
     * `with_app` / the `FrameLoop` Choreographer are valid); failures are
     * logged but never crash the service.
     */
    private fun bootstrapServicePlugin() {
        try {
            if (serviceRuntime != null && serviceInstance != null) return
            val runtime = EasePluginBridge.runtime(this)
            serviceRuntime = runtime
            // Currently the service instance loads only the OneDrive plugin
            // bundle, so the per-instance plugin id is `com.ease.onedrive`.
            // (Future multi-plugin service hosting would need a per-plugin
            // runtime or another routing layer.)
            val instance = runtime.createHeadlessInstance("com.ease.onedrive")
            serviceInstance = instance
            val pluginJs = assets.open("plugins/com.ease.onedrive/plugin.js").bufferedReader().use { it.readText() }
            instance.loadModule(pluginJs)
            val ok = EasePluginBridge.wireServiceRpc(instance.nativeHandle())
            if (!ok) {
                bridge.logRaw("error", "wireServiceRpc returned false (see logcat)")
            }
        } catch (e: Throwable) {
            bridge.logRaw("error", "service plugin bootstrap failed: ${e.message}")
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
        serviceInstance?.close()
        serviceInstance = null
        serviceRuntime?.close()
        serviceRuntime = null
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
}
