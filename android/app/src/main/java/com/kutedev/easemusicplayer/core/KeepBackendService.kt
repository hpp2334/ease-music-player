package com.kutedev.easemusicplayer.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PluginManager
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
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import javax.inject.Inject


@AndroidEntryPoint
class KeepBackendService : Service() {
    @Inject lateinit var bridge: Bridge
    @Inject lateinit var pluginRepository: PluginRepository
    @Inject lateinit var pluginManager: PluginManager
    private val _channelId: String = "EaseMusicBackendServiceChannel"

    /** Held for the service lifetime so the headless instances are not GC'd. */
    private var serviceRuntime: TurRuntime? = null
    private var serviceScope: CoroutineScope? = null
    private val serviceInstances = mutableListOf<TurInstance>()
    /** Guards [loadPluginBackends] re-entries (initial load + revision bumps). */
    private val loadMutex = Mutex()

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
     * Bring up the plugin install layer + the headless tur instances that
     * host JS plugin backends. Sequence: build the shared runtime
     * (registers `TurRpcPlugin` + `EaseMusicPlugin`), run the first-run
     * install bootstrap ([PluginManager.bootstrapDefaults] — bundled
     * WebDAV + any storage-referenced plugins), then load every *enabled*
     * plugin's backend module into a headless instance stamped with the
     * plugin's id and wire the event bus into a `Send` `RpcClient` the
     * backend can call from any thread.
     *
     * [PluginManager.revision] is collected for the service lifetime: every
     * install / uninstall / enable / disable mutation tears all instances
     * down (unwiring their service RPC entries) and reloads the enabled
     * set. The scan runs on [Dispatchers.IO]; instance creation + wiring
     * run on the main looper (where `with_app` / the `FrameLoop`
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
                    pluginManager.bootstrapDefaults()
                    loadPluginBackends()
                    pluginManager.revision.collect {
                        bridge.logRaw("info", "plugin set changed (revision $it) — reloading backends")
                        loadPluginBackends()
                    }
                } catch (e: Throwable) {
                    bridge.logRaw("error", "service plugin bootstrap failed: ${e.message}")
                }
            }
        } catch (e: Throwable) {
            bridge.logRaw("error", "service plugin bootstrap failed: ${e.message}")
        }
    }

    /** Plugin ids whose backend instance is currently live (wired into the
     *  backend context). Tracked so teardown can unwire exactly those. */
    private val loadedPluginIds = mutableListOf<String>()

    /** (Re)load the headless backend instances for all enabled plugins.
     *  Tears down any live instances first — closing each instance and
     *  unwiring its service RPC entry so storage dispatch + events for a
     *  disabled/uninstalled plugin stop at the source. */
    private suspend fun loadPluginBackends() {
        loadMutex.withLock {
            pluginRepository.scanPlugins()
            for (instance in serviceInstances) {
                runCatching { instance.close() }
            }
            serviceInstances.clear()
            for (id in loadedPluginIds.toList()) {
                runCatching { EasePluginBridge.unwireServiceRpc(id) }
            }
            loadedPluginIds.clear()
            for (plugin in pluginRepository.enabledPlugins.value) {
                val backendFile = plugin.backend ?: continue
                try {
                    val js = withContext(Dispatchers.IO) {
                        val text = pluginRepository.openPluginFile(plugin.id, backendFile)
                        checkNotNull(text) { "backend file missing: $backendFile" }
                    }
                    val instance = serviceRuntime?.createHeadlessInstance(plugin.id) ?: continue
                    serviceInstances += instance
                    loadedPluginIds += plugin.id
                    instance.loadModule(js)
                    val ok = EasePluginBridge.wireServiceRpc(instance.nativeHandle(), plugin.id)
                    if (!ok) {
                        bridge.logRaw("error", "wireServiceRpc failed for ${plugin.id} (see logcat)")
                    } else {
                        bridge.logRaw("info", "plugin backend loaded: ${plugin.id}/$backendFile")
                    }
                } catch (e: Throwable) {
                    bridge.logRaw("error", "plugin backend load failed: ${plugin.id} (${e.message})")
                }
            }
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
        for (id in loadedPluginIds.toList()) {
            runCatching { EasePluginBridge.unwireServiceRpc(id) }
        }
        loadedPluginIds.clear()
        serviceInstances.forEach { instance ->
            runCatching { instance.close() }
        }
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
