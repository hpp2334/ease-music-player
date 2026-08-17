package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import dagger.hilt.android.qualifiers.ApplicationContext
import android.content.Context
import com.kutedev.easemusicplayer.singleton.types.ArgPluginEvent
import com.kutedev.easemusicplayer.singleton.types.PluginScanInfo
import javax.inject.Inject
import javax.inject.Singleton

/**
 * One plugin's static metadata, mirroring the Rust-side scan
 * (`plugin.list`). Populated by [scanPlugins].
 *
 * [backendSourceHandle] is the module-source handle of the plugin's
 * long-lived backend module (loaded into a headless tur instance by
 * `KeepBackendService` via `loadModule(handle)`); each contribution's
 * [Contribution.viewSourceHandle] feeds a short-lived `TurView`. Handles
 * come from the runtime's shared `ModuleSourceRegistry` (registered by
 * Rust during the scan) — the JS bytes never reach Kotlin. `0` means
 * "none / not loadable" (disabled plugins, missing files, or the runtime
 * not yet bound).
 */
data class PluginManifest(
    val id: String,
    val name: String,
    val version: String,
    val description: String = "",
    val backend: String? = null,
    val backendSourceHandle: Long = 0L,
    val events: List<String> = emptyList(),
    val dashboard: List<DashboardContribution> = emptyList(),
    val storages: List<StorageContribution> = emptyList(),
    /** `false` when the user disabled the plugin in plugin management. */
    val enabled: Boolean = true,
)

/** A dashboard contribution declared in a plugin's `manifest.json`
 * (`contributions.dashboard`); each renders as an entry card on the
 * Dashboard. [viewSourceHandle] loads the standalone view page. */
data class DashboardContribution(
    val id: String,
    val title: String,
    val view: String? = null,
    val viewSourceHandle: Long = 0L,
)

/** A storage contribution declared in a plugin's `manifest.json`. */
data class StorageContribution(
    /** The storage provider id (e.g. `"onedrive"`); used as the `provider`
     * argument to `pluginOAuthUrl` / `pluginOAuthExchange`. */
    val id: String,
    /** The storage view JS filename (informational; loading goes through
     * [viewSourceHandle]). */
    val view: String? = null,
    val viewSourceHandle: Long = 0L,
)

/**
 * A discoverable storage provider built from a plugin manifest. Drives the
 * add-storage chooser ("WebDAV" + one card per provider) and, when selected,
 * the view loaded into a `TurView` — by [viewSourceHandle].
 */
data class StorageProvider(
    val pluginId: String,
    val storageId: String,
    val displayName: String,
    /** Module-source handle of the view JS, or `0` if none. */
    val viewSourceHandle: Long,
)

/**
 * A plugin dashboard card, flattened from the enabled plugins'
 * `contributions.dashboard`. Each item renders a [DashboardCard] on the
 * Dashboard page; tapping pushes the standalone view page loaded by
 * [viewSourceHandle].
 */
data class DashboardItem(
    val pluginId: String,
    val pluginName: String,
    val contributionId: String,
    val title: String,
    /** Module-source handle of the view JS, or `0` if none. */
    val viewSourceHandle: Long,
)

/**
 * Plugin runtime registry.
 *
 * Plugins are installed under `filesDir/plugins/<id>/` by the Rust-side
 * plugin manager; [scanPlugins] calls `plugin.list` on the bridge and
 * publishes the parsed manifests ([installedPlugins], [enabledPlugins]),
 * their dashboard contributions ([dashboardItems]) and storage
 * contributions ([storageProviders]). Disabled plugins are scanned but
 * excluded from the contribution flows (zero source handles).
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
     * Fetch the installed-plugin state from the Rust side (`plugin.list`)
     * and publish the manifests / dashboard items / storage providers. The
     * Rust scan also (re)registers every enabled backend/view module
     * source on the tur runtime, returning fresh handles each generation.
     * Idempotent; safe to call from a ViewModel's `init` or a service.
     * Re-run after any install / uninstall / enable / disable mutation.
     */
    suspend fun scanPlugins() {
        val result = bridge.call(BridgeMethods.Plugin.LIST).unwrapOrNull()?.payload ?: return
        _installedPlugins.value = result.plugins.map(::toManifest)
        _enabledPlugins.value = result.plugins.filter { it.enabled }.map(::toManifest)
        recomputeDashboardItems()
        recomputeStorageProviders()
    }

    private fun toManifest(info: PluginScanInfo) = PluginManifest(
        id = info.id,
        name = info.name,
        version = info.version,
        description = info.description,
        backend = info.backend,
        backendSourceHandle = info.backendSourceHandle,
        events = info.events,
        dashboard = info.dashboard.map {
            DashboardContribution(
                id = it.id,
                title = it.title,
                view = it.view,
                viewSourceHandle = it.sourceHandle,
            )
        },
        storages = info.storages.map {
            StorageContribution(
                id = it.id,
                view = it.view,
                viewSourceHandle = it.sourceHandle,
            )
        },
        enabled = info.enabled,
    )

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
                        viewSourceHandle = d.viewSourceHandle,
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
                        viewSourceHandle = s.viewSourceHandle,
                    )
                )
            }
        }
        _storageProviders.value = out
    }
}
