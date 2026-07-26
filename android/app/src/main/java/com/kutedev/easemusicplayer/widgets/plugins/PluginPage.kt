package com.kutedev.easemusicplayer.widgets.plugins

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.material3.Text

/**
 * Top-level router for `RoutePlugin(pluginId, viewId)`. Dispatches to the
 * right composable per plugin.
 *
 * Plugins with a JS entry (`main` field in their manifest) render through
 * [TurPluginPage] — a tur engine view that imports `ease:storage` and
 * `tur:std`. The plugin JS owns all view + biz logic; the host stays
 * decoupled.
 *
 * Currently every built-in plugin routes through [TurPluginPage]; the
 * fallback stub only fires for unknown plugin ids (e.g. a partially
 * installed plugin whose manifest declares a view but whose assets are
 * missing).
 */
@Composable
fun PluginPage(
    pluginId: String,
    viewId: String,
    scaffoldPadding: PaddingValues,
) {
    TurPluginPage(
        pluginId = pluginId,
        viewId = viewId,
        scaffoldPadding = scaffoldPadding,
    )
}

@Composable
private fun UnknownPluginPage(pluginId: String, viewId: String) {
    Text("Unknown plugin: $pluginId / $viewId")
}
