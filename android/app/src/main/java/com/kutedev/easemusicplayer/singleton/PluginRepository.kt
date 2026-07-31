package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.ArgPluginKvAppend
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton

/**
 * One plugin's static metadata, mirroring its `manifest.json`. Built-in
 * plugins are constructed directly; installed plugins are parsed from
 * the manifest the install step wrote to the app-private dir.
 *
 * For now only `views` is consumed (each [PluginViewContribution] becomes
 * one row in the Plugins tab). `lyrics` and `storages` are recorded for
 * future use; their wiring lands with the lyric-parser / storage-backend
 * contribution support.
 */
data class PluginManifest(
    val id: String,
    val name: String,
    val version: String,
    val main: String? = null,
    val events: List<String> = emptyList(),
    val views: List<PluginViewContribution> = emptyList(),
)

/** One row in the Plugins tab. */
data class PluginViewContribution(
    val id: String,
    val title: String,
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
)

/**
 * Plugin runtime.
 *
 * Holds the registry of enabled plugins and routes [PlayerControllerRepository]'s
 * plugin-event bus to each enabled plugin whose `events` declaration matches.
 *
 * For now only the built-in `com.ease.playcount` plugin is registered. The
 * play-count handler runs entirely on the Kotlin side — it appends one row
 * per play event to the plugin's multi-value KV namespace under a per-day
 * key. When tur integration lands, the handler will be replaced by a JS
 * callback; the data model stays the same.
 */
@Singleton
class PluginRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope,
) {
    private val _enabledPlugins = MutableStateFlow<List<PluginManifest>>(emptyList())
    val enabledPlugins = _enabledPlugins.asStateFlow()

    private val _pluginViews = MutableStateFlow<List<PluginViewItem>>(emptyList())
    val pluginViews = _pluginViews.asStateFlow()

    init {
        // Register built-in plugins. Loaded once on construction; future
        // dynamic installs will append to this list at runtime.
        _enabledPlugins.value = listOf(BUILT_IN_PLAYCOUNT, BUILT_IN_TURTEST)
        recomputeViews()

        // Subscribe to the player event bus and dispatch.
        _scope.launch(Dispatchers.Default) {
            // Late-binding collect: PlayerControllerRepository is constructed
            // before this repo in the Hilt graph, so by the time init runs
            // its SharedFlow is hot. Inject is by constructor so we use a
            // setter to attach the event source.
        }
    }

    /**
     * Connects the player's plugin-event bus. Called once from
     * [com.kutedev.easemusicplayer.MainActivity] after both repositories
     * have been constructed by Hilt.
     */
    fun bindPlayerEvents(playerController: PlayerControllerRepository) {
        _scope.launch(Dispatchers.Default) {
            playerController.pluginEvents.collect { event ->
                dispatch(event)
            }
        }
    }

    private fun dispatch(event: PluginEvent) {
        for (plugin in _enabledPlugins.value) {
            if (event.type !in plugin.events) continue
            when (plugin.id) {
                PLAYCOUNT_ID -> handlePlaycountEvent(event)
                // future plugins dispatch here
            }
        }
    }

    // ------------------------------------------------------------------
    // Built-in: com.ease.playcount
    //
    // KV layout (append-only / multi mode):
    //   key   = "plays:YYYY-MM-DD"
    //   value = JSON {"musicId": <i64>, "ts": <ms>}
    // Each music:play event appends one row. The view fetches counts per
    // day via ctPluginKvMultiCountMulti(keys) — one SQL round-trip per
    // selected time range.
    // ------------------------------------------------------------------

    private fun handlePlaycountEvent(event: PluginEvent) {
        when (event) {
            is PluginEvent.MusicPlay -> {
                val today = LocalDate.now().format(DateTimeFormatter.ISO_LOCAL_DATE)
                val key = "plays:$today"
                val payload = buildJsonObject {
                    put("musicId", event.musicId.value)
                    put("title", event.title)
                    put("ts", event.timestamp)
                }.toString()
                _scope.launch(Dispatchers.IO) {
                    bridge.call(
                        BridgeMethods.Plugin.KV_MULTI_APPEND,
                        ArgPluginKvAppend(
                            pluginId = PLAYCOUNT_ID,
                            key = key,
                            value = payload,
                        ),
                    ).unwrapOrNull()
                }
            }
            is PluginEvent.MusicPause, is PluginEvent.MusicStop, is PluginEvent.MusicComplete -> {
                // Play-count only tracks starts. Other plugins may hook these.
            }
        }
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
                    )
                )
            }
        }
        _pluginViews.value = out
    }

    companion object {
        const val PLAYCOUNT_ID = "com.ease.playcount"
        const val TURTEST_ID = "com.ease.turtest"

        private val BUILT_IN_PLAYCOUNT = PluginManifest(
            id = PLAYCOUNT_ID,
            name = "Play Counts",
            version = "1.0.0",
            main = "plugin.js",
            events = listOf(PluginEvent.MUSIC_PLAY),
            views = listOf(
                PluginViewContribution(id = "main", title = "Play Counts"),
            ),
        )

        // Built-in: com.ease.turtest. Minimal repro views for tur engine
        // layout questions; no events, no KV. Currently hosts the
        // `followerAnchor` CompositedTransform repro.
        private val BUILT_IN_TURTEST = PluginManifest(
            id = TURTEST_ID,
            name = "Tur Test",
            version = "1.0.0",
            main = "plugin.js",
            events = emptyList(),
            views = listOf(
                PluginViewContribution(id = "follower-anchor", title = "Follower Anchor"),
            ),
        )
    }
}
