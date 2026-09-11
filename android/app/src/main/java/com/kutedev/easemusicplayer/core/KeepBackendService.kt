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
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.PluginRuntimeHost
import com.kutedev.easemusicplayer.turintegration.TurInstance
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import javax.inject.Inject


@AndroidEntryPoint
class KeepBackendService : Service() {
    @Inject lateinit var bridge: Bridge
    @Inject lateinit var pluginRepository: PluginRepository
    @Inject lateinit var pluginManager: PluginManager
    @Inject lateinit var pluginRuntimeHost: PluginRuntimeHost
    private val _channelId: String = "EaseMusicBackendServiceChannel"

    /** Held for the service lifetime so the headless instances are not GC'd. */
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
     * host JS plugin backends. Sequence: start the shared runtime via
     * [PluginRuntimeHost] (explicit create → `bindPluginRuntime`, logged —
     * it registers `TurRpcPlugin` + `EaseMusicPlugin` bound to *this*
     * backend instance), run the first-run install bootstrap
     * ([PluginManager.bootstrapDefaults] — bundled WebDAV + any
     * storage-referenced plugins), then load every *enabled* plugin's
     * backend module into a headless instance stamped with the plugin's id
     * and wire the event bus into a `Send` `RpcClient` the backend can call
     * from any thread.
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
        if (serviceScope != null) return
        try {
            pluginRuntimeHost.start(this)
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
                    Log.e(TAG, "service plugin bootstrap failed", e)
                }
            }
        } catch (e: Throwable) {
            bridge.logRaw("error", "service plugin bootstrap failed: ${e.message}")
            Log.e(TAG, "service plugin bootstrap failed", e)
        }
    }

    /** Plugin ids whose backend instance is currently live (wired into the
     *  backend context). Tracked so teardown can unwire exactly those. */
    private val loadedPluginIds = mutableListOf<String>()

    /** (Re)load the headless backend instances for all enabled plugins.
     *  Tears down any live instances first — closing each instance and
     *  unwiring its service RPC entry so storage dispatch + events for a
     *  disabled/uninstalled plugin stop at the source. Backend modules
     *  load by **source handle** (registered on the runtime by the
     *  Rust-side `plugin.list` scan — tur #198); no JS string crosses
     *  JNI. */
    private suspend fun loadPluginBackends() {
        loadMutex.withLock {
            pluginRepository.scanPlugins()
            for (instance in serviceInstances) {
                runCatching { instance.close() }
            }
            serviceInstances.clear()
            val backendHandle = bridge.getBackendId()
            for (id in loadedPluginIds.toList()) {
                runCatching { EasePluginBridge.unwireServiceRpc(backendHandle, id) }
            }
            loadedPluginIds.clear()
            val runtime = pluginRuntimeHost.runtimeOrNull()
            if (runtime == null) {
                bridge.logRaw("error", "loadPluginBackends: plugin runtime not running — no backends loaded")
                Log.e(TAG, "loadPluginBackends: plugin runtime not running")
                return@withLock
            }
            for (plugin in pluginRepository.enabledPlugins.value) {
                val sourceHandle = plugin.backendSourceHandle
                if (plugin.backend == null || sourceHandle == 0L) {
                    bridge.logRaw(
                        "error",
                        "plugin backend skipped (no source handle): ${plugin.id} — " +
                            "see the plugin scan warnings in the log",
                    )
                    Log.e(TAG, "plugin backend skipped (no source handle): ${plugin.id}")
                    continue
                }
                try {
                    val instance = runtime.createHeadlessInstance(plugin.id)
                    serviceInstances += instance
                    loadedPluginIds += plugin.id
                    instance.loadModule(sourceHandle)
                    val ok = EasePluginBridge.wireServiceRpc(backendHandle, instance.nativeHandle(), plugin.id)
                    if (!ok) {
                        bridge.logRaw("error", "wireServiceRpc failed for ${plugin.id} (see logcat)")
                    } else {
                        bridge.logRaw("info", "plugin backend loaded: ${plugin.id}/${plugin.backend}")
                    }
                } catch (e: Throwable) {
                    bridge.logRaw("error", "plugin backend load failed: ${plugin.id} (${e.message})")
                    Log.e(TAG, "plugin backend load failed: ${plugin.id}", e)
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
        val backendHandle = bridge.getBackendId()
        for (id in loadedPluginIds.toList()) {
            runCatching { EasePluginBridge.unwireServiceRpc(backendHandle, id) }
        }
        loadedPluginIds.clear()
        serviceInstances.forEach { instance ->
            runCatching { instance.close() }
        }
        serviceInstances.clear()
        serviceScope?.cancel()
        serviceScope = null
        // Unbind + destroy the runtime while the backend handle is still
        // resolvable — AFTER bridge.destroy() the Rust-side binding could
        // not be cleared anymore.
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
