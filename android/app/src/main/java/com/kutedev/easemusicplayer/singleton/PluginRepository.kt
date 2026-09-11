package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import dagger.hilt.android.qualifiers.ApplicationContext
import android.content.Context
import com.kutedev.easemusicplayer.singleton.types.ArgPluginEvent
import com.kutedev.easemusicplayer.singleton.types.PluginScanInfo
import com.kutedev.easemusicplayer.viewmodels.setLyricExts
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
    val name: LocalizedText,
    val version: String,
    val description: LocalizedText = LocalizedText(""),
    val backend: String? = null,
    val backendSourceHandle: Long = 0L,
    val events: List<String> = emptyList(),
    /** Base64 plugin icon bytes; the built-in extension glyph shows when absent. */
    val iconData: String? = null,
    val dashboard: List<DashboardContribution> = emptyList(),
    val storages: List<StorageContribution> = emptyList(),
    val lyricParsers: List<LyricParserContribution> = emptyList(),
    /** `false` when the user disabled the plugin in plugin management. */
    val enabled: Boolean = true,
)

/** A dashboard contribution declared in a plugin's `manifest.json`
 * (`contributions.dashboard`); each renders as an entry card on the
 * Dashboard. [viewSourceHandle] loads the standalone view page. */
data class DashboardContribution(
    val id: String,
    /** Falls back to the plugin name when the manifest omitted `title`. */
    val title: LocalizedText? = null,
    /** Short card subtitle. */
    val desc: LocalizedText? = null,
    /** Icon file name (informational; rendering goes through [iconData]). */
    val icon: String? = null,
    /** Base64 icon bytes, or `null` when the file failed validation. */
    val iconData: String? = null,
    val view: String? = null,
    val viewSourceHandle: Long = 0L,
)

/** A storage contribution declared in a plugin's `manifest.json`. */
data class StorageContribution(
    /** The storage provider id (e.g. `"onedrive"`); used as the `provider`
     * argument to `pluginOAuthUrl` / `pluginOAuthExchange`. */
    val id: String,
    /** Falls back to the plugin name when the manifest omitted `title`. */
    val title: LocalizedText? = null,
    val desc: LocalizedText? = null,
    val icon: String? = null,
    val iconData: String? = null,
    /** The storage view JS filename (informational; loading goes through
     * [viewSourceHandle]). */
    val view: String? = null,
    val viewSourceHandle: Long = 0L,
)

/** A `contributions.lyricParsers` entry: one plugin-provided lyric
 * format family (headless — parsing rides the backend's `lyric:parse`
 * host-RPC handler). Rendered by the Lyric Parser settings page; the
 * extensions also drive the import/browse lyric filters. */
data class LyricParserContribution(
    val pluginId: String,
    val parserId: String,
    /** The owning plugin's name — the parser-title fallback. */
    val pluginName: LocalizedText,
    /** Falls back to the plugin name when the manifest omitted `title`. */
    val title: LocalizedText? = null,
    val desc: LocalizedText? = null,
    val iconData: String? = null,
    val extensions: List<String> = emptyList(),
)

/**
 * A discoverable storage provider built from a plugin manifest. Drives the
 * add-storage chooser ("WebDAV" + one card per provider) and, when selected,
 * the view loaded into a `TurView` — by [viewSourceHandle].
 */
data class StorageProvider(
    val pluginId: String,
    val storageId: String,
    val displayName: LocalizedText,
    val desc: LocalizedText? = null,
    val iconData: String? = null,
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
    val pluginName: LocalizedText,
    val contributionId: String,
    val title: LocalizedText,
    /** Card subtitle; the raw plugin id shows when absent. */
    val desc: LocalizedText? = null,
    /** Base64 icon bytes; the built-in extension glyph shows when absent. */
    val iconData: String? = null,
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
    easeBackend: EaseBackend,
) {
    init {
        // Rust-driven refresh: every backend reload (install / uninstall /
        // enable / disable / bind) emits PLUGINS_CHANGED; rescan debounced
        // (collectLatest + delay) so bursts coalesce into one `plugin.list`.
        _scope.launch(Dispatchers.Default) {
            easeBackend.signals.collectLatest {
                if (it is BackendSignal.PluginsChanged) {
                    delay(250)
                    bridge.logRaw(
                        "info",
                        "plugins changed (generation ${it.generation}) — rescanning",
                    )
                    scanPlugins()
                }
            }
        }
    }

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

    private val _lyricParsers = MutableStateFlow<List<LyricParserContribution>>(emptyList())
    /** Plugin-declared lyric parsers (enabled plugins only), populated by
     *  [scanPlugins]. Empty until a parser plugin is installed + enabled —
     *  there is no built-in parser. */
    val lyricParsers = _lyricParsers.asStateFlow()

    private val _lyricExtensions = MutableStateFlow<Set<String>>(emptySet())
    /** Every extension claimable by an enabled plugin parser (no leading
     *  dot); derived from [lyricParsers] by [scanPlugins]. Feeds the
     *  import/browse lyric filters ([setLyricExts]). */
    val lyricExtensions = _lyricExtensions.asStateFlow()

    private val _lyricParserSelection = MutableStateFlow<Map<String, String>>(emptyMap())
    /** User's per-extension parser picks (extension →
     *  `"<pluginId>:<parserId>"`); absent entry = Auto. Updated by
     *  [scanPlugins] and [PluginManager.setLyricParserSelection]. */
    val lyricParserSelection = _lyricParserSelection.asStateFlow()

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
        for (warning in result.warnings) {
            bridge.logRaw("error", "plugin scan: $warning")
        }
        _installedPlugins.value = result.plugins.map(::toManifest)
        _enabledPlugins.value = result.plugins.filter { it.enabled }.map(::toManifest)
        recomputeDashboardItems()
        recomputeStorageProviders()
        recomputeLyricParsers()
        _lyricParserSelection.value = result.lyricParserSelection
    }

    private fun toManifest(info: PluginScanInfo) = PluginManifest(
        id = info.id,
        name = info.name,
        version = info.version,
        description = info.description,
        backend = info.backend,
        backendSourceHandle = info.backendSourceHandle,
        events = info.events,
        iconData = info.iconData,
        dashboard = info.dashboard.map {
            DashboardContribution(
                id = it.id,
                title = it.title,
                desc = it.desc,
                icon = it.icon,
                iconData = it.iconData,
                view = it.view,
                viewSourceHandle = it.sourceHandle,
            )
        },
        storages = info.storages.map {
            StorageContribution(
                id = it.id,
                title = it.title,
                desc = it.desc,
                icon = it.icon,
                iconData = it.iconData,
                view = it.view,
                viewSourceHandle = it.sourceHandle,
            )
        },
        lyricParsers = info.lyricParsers.map {
            LyricParserContribution(
                pluginId = info.id,
                parserId = it.id,
                pluginName = info.name,
                title = it.title,
                desc = it.desc,
                iconData = it.iconData,
                extensions = it.extensions,
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
                        // No explicit contribution title → the plugin's name.
                        title = d.title ?: p.name,
                        desc = d.desc,
                        iconData = d.iconData,
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
                        // No explicit contribution title → the plugin's name.
                        displayName = s.title ?: p.name,
                        desc = s.desc,
                        iconData = s.iconData,
                        viewSourceHandle = s.viewSourceHandle,
                    )
                )
            }
        }
        _storageProviders.value = out
    }

    private fun recomputeLyricParsers() {
        // `toManifest` already maps wire → contribution (plugin names
        // attached); flatten in scan order (plugins sorted by id,
        // contributions in manifest order = default dispatch order).
        val out = _enabledPlugins.value.flatMap { p -> p.lyricParsers }
        _lyricParsers.value = out
        _lyricExtensions.value = out.flatMap { it.extensions }.toSet()
        // Feed the extension-classification helper used by the import /
        // browse filters (see StoragesVM.kt).
        setLyricExts(_lyricExtensions.value)
    }

    /** Merge a selection update from [PluginManager.setLyricParserSelection]
     *  without a full rescan (selection changes never bump the revision). */
    fun updateLyricParserSelection(selection: Map<String, String>) {
        _lyricParserSelection.value = selection
    }
}
