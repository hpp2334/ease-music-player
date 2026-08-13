package com.kutedev.easemusicplayer.singleton

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import javax.inject.Inject
import javax.inject.Singleton
import dagger.hilt.android.qualifiers.ApplicationContext
import com.kutedev.easemusicplayer.singleton.types.ArgPluginEvent

/**
 * One plugin's static metadata, mirroring its `manifest.json` under
 * `assets/plugins/<id>/`. Populated by [PluginRepository.scanPlugins].
 *
 * [backend] is the plugin's long-lived module (loaded into a headless tur
 * instance by `KeepBackendService`); each contribution's `view` is a
 * short-lived module loaded per rendered page.
 */
data class PluginManifest(
    val id: String,
    val name: String,
    val version: String,
    val backend: String? = null,
    val events: List<String> = emptyList(),
    val views: List<PluginViewContribution> = emptyList(),
    val storages: List<StorageContribution> = emptyList(),
)

/** One row in the Plugins tab. */
data class PluginViewContribution(
    val id: String,
    val title: String,
    /** Filename of the view JS (e.g. `"view.js"`), relative to the plugin's
     *  asset dir. `null` if the plugin declares no view file. */
    val view: String? = null,
)

/** A storage contribution declared in a plugin's `manifest.json`. */
data class StorageContribution(
    /** The storage provider id (e.g. `"onedrive"`); used as the `provider`
     *  argument to `pluginOAuthUrl` / `pluginOAuthExchange`. */
    val id: String,
    /** Filename of the storage view JS (e.g. `"view.js"`), relative to the
     *  plugin's asset dir. `null` if the plugin declares no view. */
    val view: String? = null,
)

/**
 * A discoverable storage provider built from a plugin manifest. Drives the
 * add-storage chooser ("WebDAV" + one card per provider) and, when selected,
 * the view JS path loaded into a `TurView`.
 */
data class StorageProvider(
    val pluginId: String,
    val storageId: String,
    val displayName: String,
    /** Absolute asset path of the view JS, or null if none declared. */
    val viewAssetPath: String?,
)

/**
 * Flat `(pluginId, pluginName, view)` tuple emitted by [pluginViews].
 * Drives the Plugins subpage list — each item navigates to
 * `RoutePlugin(pluginId, viewId)`.
 */
data class PluginViewItem(
    val pluginId: String,
    val pluginName: String,
    val viewId: String,
    val viewTitle: String,
    /** Absolute asset path of the view JS, or null if none declared. */
    val viewAssetPath: String?,
)

/**
 * Plugin runtime registry.
 *
 * Scans `assets/plugins/<id>/manifest.json` ([scanPlugins]) and publishes the
 * parsed manifests ([enabledPlugins]), their view contributions
 * ([pluginViews]) and storage contributions ([storageProviders]).
 *
 * Routes [PlayerControllerRepository]'s plugin-event bus to each plugin
 * whose `events` declaration matches: the host calls `plugin.event` on the
 * bridge, which dispatches to the plugin's backend JS module via its
 * headless tur instance's RpcClient. No per-plugin logic lives on the
 * Kotlin side — backends register `tur:rpc` handlers for the event types
 * they declare.
 */
@Singleton
class PluginRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope,
    @ApplicationContext private val context: Context,
) {
    private val _enabledPlugins = MutableStateFlow<List<PluginManifest>>(emptyList())
    val enabledPlugins = _enabledPlugins.asStateFlow()

    private val _pluginViews = MutableStateFlow<List<PluginViewItem>>(emptyList())
    val pluginViews = _pluginViews.asStateFlow()

    private val _storageProviders = MutableStateFlow<List<StorageProvider>>(emptyList())
    /** Plugin-declared storage providers, populated by [scanPlugins]. */
    val storageProviders = _storageProviders.asStateFlow()

    /**
     * Connects the player's plugin-event bus. Called once from
     * [com.kutedev.easemusicplayer.MainActivity] after both repositories
     * have been constructed by Hilt.
     */
    fun bindPlayerEvents(playerController: PlayerControllerRepository) {
        _scope.launch(Dispatchers.Default) {
            playerController.pluginEvents.collect { event ->
                val payload = event.toJsonElement()
                for (plugin in _enabledPlugins.value) {
                    if (event.type in plugin.events) {
                        bridge.call(
                            BridgeMethods.Plugin.EVENT,
                            ArgPluginEvent(plugin.id, event.type, payload),
                        ).unwrapOrNull()
                    }
                }
            }
        }
    }

    /**
     * Scan each `assets/plugins/<dir>/manifest.json` and publish the parsed
     * manifests / views / storage providers. Idempotent; safe to call from a
     * ViewModel's `init` or a service. Runs on [Dispatchers.IO] (asset +
     * JSON parse).
     */
    suspend fun scanPlugins() {
        val manifests = withContext(Dispatchers.IO) {
            val out = mutableListOf<PluginManifest>()
            val dirs = runCatching { context.assets.list("plugins") }.getOrNull() ?: emptyArray()
            for (dir in dirs) {
                val manifestText = runCatching {
                    context.assets.open("plugins/$dir/manifest.json").bufferedReader().use { it.readText() }
                }.getOrNull() ?: continue
                val m = runCatching { JSONObject(manifestText) }.getOrNull() ?: continue
                val pluginId = m.optString("id", dir)
                val backend = if (m.has("backend") && !m.isNull("backend")) m.getString("backend") else null
                val events = m.optJSONArray("events")?.let { arr ->
                    (0 until arr.length()).map { arr.getString(it) }
                } ?: emptyList()
                val contributions = m.optJSONObject("contributions")
                val views = contributions?.optJSONArray("views")?.let { arr ->
                    (0 until arr.length()).mapNotNull { i ->
                        val v = arr.optJSONObject(i) ?: return@mapNotNull null
                        val vid = v.optString("id")
                        if (vid.isBlank()) return@mapNotNull null
                        PluginViewContribution(
                            id = vid,
                            title = v.optString("title", vid),
                            view = if (v.has("view") && !v.isNull("view")) v.getString("view") else null,
                        )
                    }
                } ?: emptyList()
                val storages = contributions?.optJSONArray("storages")?.let { arr ->
                    (0 until arr.length()).mapNotNull { i ->
                        val s = arr.optJSONObject(i) ?: return@mapNotNull null
                        val sid = s.optString("id")
                        if (sid.isBlank()) return@mapNotNull null
                        StorageContribution(
                            id = sid,
                            view = if (s.has("view") && !s.isNull("view")) s.getString("view") else null,
                        )
                    }
                } ?: emptyList()
                out.add(
                    PluginManifest(
                        id = pluginId,
                        name = m.optString("name", pluginId),
                        version = m.optString("version", "0.0.0"),
                        backend = backend,
                        events = events,
                        views = views,
                        storages = storages,
                    )
                )
            }
            out
        }
        _enabledPlugins.value = manifests
        recomputeViews()
        recomputeStorageProviders()
    }

    private fun recomputeViews() {
        val out = mutableListOf<PluginViewItem>()
        for (p in _enabledPlugins.value) {
            for (v in p.views) {
                out.add(
                    PluginViewItem(
                        pluginId = p.id,
                        pluginName = p.name,
                        viewId = v.id,
                        viewTitle = v.title,
                        viewAssetPath = v.view?.let { "plugins/${p.id}/$it" },
                    )
                )
            }
        }
        _pluginViews.value = out
    }

    private fun recomputeStorageProviders() {
        val out = mutableListOf<StorageProvider>()
        for (p in _enabledPlugins.value) {
            for (s in p.storages) {
                out.add(
                    StorageProvider(
                        pluginId = p.id,
                        storageId = s.id,
                        displayName = p.name,
                        viewAssetPath = s.view?.let { "plugins/${p.id}/$it" },
                    )
                )
            }
        }
        _storageProviders.value = out
    }
}
