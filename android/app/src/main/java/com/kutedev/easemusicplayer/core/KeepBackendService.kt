package com.kutedev.easemusicplayer.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurInstance
import com.kutedev.easemusicplayer.turintegration.TurRuntime
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject


@AndroidEntryPoint
class KeepBackendService : Service() {
    @Inject lateinit var bridge: Bridge
    @Inject lateinit var pluginRepository: PluginRepository
    private val _channelId: String = "EaseMusicBackendServiceChannel"

    /** Held for the service lifetime so the headless instances are not GC'd. */
    private var serviceRuntime: TurRuntime? = null
    private var serviceScope: CoroutineScope? = null
    private val serviceInstances = mutableListOf<TurInstance>()

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
     * Bring up the headless tur instances that host JS plugin backends
     * (every manifest declaring a `backend` field — e.g. the OneDrive
     * storage provider, the play-count event hook). Sequence: build the
     * shared runtime (registers `TurRpcPlugin` + `EaseMusicPlugin`), scan
     * `assets/plugins/<id>/manifest.json`, then for each backend spawn a
     * headless instance stamped with the plugin's id, load its module, and
     * wire the event bus into a `Send` `RpcClient` the backend can call from
     * any thread. The scan runs on [Dispatchers.IO]; instance creation +
     * wiring run on the main looper (where `with_app` / the `FrameLoop`
     * Choreographer are valid); failures are logged but never crash the
     * service.
     */
    private fun bootstrapServicePlugin() {
        try {
            if (serviceRuntime != null) return
            val runtime = EasePluginBridge.runtime(this)
            serviceRuntime = runtime
            val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
            serviceScope = scope
            scope.launch {
                try {
                    pluginRepository.scanPlugins()
                    for (plugin in pluginRepository.enabledPlugins.value) {
                        val backendFile = plugin.backend ?: continue
                        val js = withContext(Dispatchers.IO) {
                            assets.open("plugins/${plugin.id}/$backendFile").bufferedReader().use { it.readText() }
                        }
                        val instance = runtime.createHeadlessInstance(plugin.id)
                        serviceInstances += instance
                        instance.loadModule(js)
                        val ok = EasePluginBridge.wireServiceRpc(instance.nativeHandle(), plugin.id)
                        if (!ok) {
                            bridge.logRaw("error", "wireServiceRpc failed for ${plugin.id} (see logcat)")
                        } else {
                            bridge.logRaw("info", "plugin backend loaded: ${plugin.id}/$backendFile")
                        }
                    }
                } catch (e: Throwable) {
                    bridge.logRaw("error", "service plugin bootstrap failed: ${e.message}")
                }
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
        serviceInstances.forEach { it.close() }
        serviceInstances.clear()
        serviceScope?.cancel()
        serviceScope = null
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
