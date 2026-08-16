package com.kutedev.easemusicplayer.singleton

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton
import dagger.hilt.android.qualifiers.ApplicationContext
import com.kutedev.easemusicplayer.singleton.types.ArgPluginEvent

/**
 * One plugin's static metadata, mirroring its `manifest.json` under
 * `filesDir/plugins/<id>/`. Populated by [PluginRepository.scanPlugins].
 *
 * [backend] is the plugin's long-lived module (loaded into a headless tur
 * instance by `KeepBackendService`); each contribution's `view` is a
 * short-lived module loaded per rendered page.
 */
data class PluginManifest(
    val id: String,
    val name: String,
    val version: String,
    val description: String = "",
    val backend: String? = null,
    val events: List<String> = emptyList(),
    val dashboard: List<DashboardContribution> = emptyList(),
    val storages: List<StorageContribution> = emptyList(),
    /** `false` when the user disabled the plugin in plugin management. */
    val enabled: Boolean = true,
)

/** A dashboard-card contribution declared in a plugin's `manifest.json`
 * (`contributions.dashboard`); each renders as a card on the Dashboard. */
data class DashboardContribution(
    val id: String,
    val title: String,
    /** Filename of the view JS (e.g. `"view.js"`), relative to the
     *  plugin's install dir. `null` if the contribution declares no file. */
    val view: String? = null,
)

/** A storage contribution declared in a plugin's `manifest.json`. */
data class StorageContribution(
    /** The storage provider id (e.g. `"onedrive"`); used as the `provider`
     *  argument to `pluginOAuthUrl` / `pluginOAuthExchange`. */
    val id: String,
    /** Filename of the storage view JS (e.g. `"view.js"`), relative to the
     *  plugin's install dir. `null` if the plugin declares no view. */
    val view: String? = null,
)

/**
 * A discoverable storage provider built from a plugin manifest. Drives the
 * add-storage chooser ("WebDAV" + one card per provider) and, when selected,
 * the view JS loaded into a `TurView`.
 */
data class StorageProvider(
    val pluginId: String,
    val storageId: String,
    val displayName: String,
    /** View JS filename inside the plugin's install dir, or null if none. */
    val viewFile: String?,
)

/**
 * A plugin dashboard card, flattened from the enabled plugins'
 * `contributions.dashboard`. Each item renders a [DashboardCard] on the
 * Dashboard page.
 */
data class DashboardItem(
    val pluginId: String,
    val pluginName: String,
    val contributionId: String,
    val title: String,
    /** View JS filename inside the plugin's install dir, or null if none. */
    val viewFile: String?,
)

/**
 * Plugin runtime registry.
 *
 * Plugins are installed as folders under `filesDir/plugins/<id>/` (see
 * [PluginManager]); [scanPlugins] walks their `manifest.json` files and
 * publishes the parsed manifests ([installedPlugins], [enabledPlugins]),
 * their dashboard contributions ([dashboardItems]) and storage
 * contributions ([storageProviders]). Disabled plugins are scanned but
 * excluded from the contribution flows.
 *
 * Routes [PlayerControllerRepository]'s plugin-event bus to each enabled
 * plugin whose `events` declaration matches: the host calls `plugin.event`
 * on the bridge, which dispatches to the plugin's backend JS module via its
 * headless tur instance's RpcClient. No per-plugin logic lives on the
 * Kotlin side — backends register `tur:rpc` handlers for the event types
 * they declare.
 */
@Singleton
class PluginRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope,
    private val stateStore: PluginStateStore,
    @ApplicationContext private val context: Context,
) {
    private val _installedPlugins = MutableStateFlow<List<PluginManifest>>(emptyList())
    val installedPlugins = _installedPlugins.asStateFlow()

    private val _enabledPlugins = MutableStateFlow<List<PluginManifest>>(emptyList())
    val enabledPlugins = _enabledPlugins.asStateFlow()

    private val _dashboardItems = MutableStateFlow<List<DashboardItem>>(emptyList())
    val dashboardItems = _dashboardItems.asStateFlow()

    private val _storageProviders = MutableStateFlow<List<StorageProvider>>(emptyList())
    /** Plugin-declared storage providers (enabled plugins only), populated
     *  by [scanPlugins]. */
    val storageProviders = _storageProviders.asStateFlow()

    /** The installed-plugin root (`filesDir/plugins`). */
    fun pluginsRoot(): File = File(context.filesDir, "plugins")

    /** Read one file from an installed plugin's dir (e.g. `backend.js`,
     *  `view.js`) as UTF-8 text. Null if the plugin or file is missing. */
    suspend fun openPluginFile(pluginId: String, fileName: String): String? =
        withContext(Dispatchers.IO) {
            runCatching {
                File(pluginsRoot(), "$pluginId/$fileName").readText()
            }.getOrNull()
        }

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
     * Scan each `filesDir/plugins/<dir>/manifest.json` and publish the
     * parsed manifests / dashboard items / storage providers. Idempotent;
     * safe to call from a ViewModel's `init` or a service. Runs on
     * [Dispatchers.IO] (file + JSON parse). Re-run after any install /
     * uninstall / enable / disable mutation.
     */
    suspend fun scanPlugins() {
        val manifests = withContext(Dispatchers.IO) {
            val state = stateStore.read()
            val out = mutableListOf<PluginManifest>()
            val dirs = runCatching { pluginsRoot().listFiles() }.getOrNull() ?: emptyArray()
            for (dir in dirs.sortedBy { it.name }) {
                if (!dir.isDirectory) continue
                val manifestFile = File(dir, "manifest.json")
                if (!manifestFile.isFile) continue
                val manifestText = runCatching { manifestFile.readText() }.getOrNull() ?: continue
                val m = runCatching { JSONObject(manifestText) }.getOrNull() ?: continue
                val pluginId = m.optString("id", dir.name)
                val backend = if (m.has("backend") && !m.isNull("backend")) m.getString("backend") else null
                val events = m.optJSONArray("events")?.let { arr ->
                    (0 until arr.length()).map { arr.getString(it) }
                } ?: emptyList()
                val contributions = m.optJSONObject("contributions")
                val dashboard = contributions?.optJSONArray("dashboard")?.let { arr ->
                    (0 until arr.length()).mapNotNull { i ->
                        val v = arr.optJSONObject(i) ?: return@mapNotNull null
                        val vid = v.optString("id")
                        if (vid.isBlank()) return@mapNotNull null
                        DashboardContribution(
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
                        description = m.optString("description", ""),
                        backend = backend,
                        events = events,
                        dashboard = dashboard,
                        storages = storages,
                        enabled = state.enabled[pluginId] ?: true,
                    )
                )
            }
            out
        }
        _installedPlugins.value = manifests
        _enabledPlugins.value = manifests.filter { it.enabled }
        recomputeDashboardItems()
        recomputeStorageProviders()
    }

    private fun recomputeDashboardItems() {
        val out = mutableListOf<DashboardItem>()
        for (p in _enabledPlugins.value) {
            for (d in p.dashboard) {
                out.add(
                    DashboardItem(
                        pluginId = p.id,
                        pluginName = p.name,
                        contributionId = d.id,
                        title = d.title,
                        viewFile = d.view,
                    )
                )
            }
        }
        _dashboardItems.value = out
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
                        viewFile = s.view,
                    )
                )
            }
        }
        _storageProviders.value = out
    }
}
