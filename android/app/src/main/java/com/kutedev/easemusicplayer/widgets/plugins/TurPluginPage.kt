package com.kutedev.easemusicplayer.widgets.plugins

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.shape.RoundedCornerShape
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurView

private val pluginsPaddingX = 24.dp

/**
 * Generic plugin-view page. Looks up the plugin's `main` JS source from
 * the app's `assets/plugins/<pluginId>/` directory and renders it via a
 * [TurView]. The plugin JS owns all view logic and data access via the
 * `ease` and `tur:std` modules — the host stays decoupled from any
 * plugin's biz logic.
 *
 * `pluginId` selects the asset subdirectory AND is stamped into the
 * instance's per-instance data slot so `ease:*` bridge fns resolve the
 * calling plugin from Rust; `viewId` is currently not branched on at the
 * host level (the plugin JS itself can route between its own declared
 * views).
 */
@Composable
fun TurPluginPage(
    pluginId: String,
    viewId: String,
    scaffoldPadding: PaddingValues,
) {
    val context = LocalContext.current
    val navController = LocalNavController.current
    var jsSource by remember(pluginId) { mutableStateOf<String?>(null) }
    var loadError by remember(pluginId) { mutableStateOf<String?>(null) }

    LaunchedEffect(pluginId) {
        try {
            // Built-in plugins ship in assets/plugins/<id>/plugin.js. For
            // installed (zip) plugins this lookup will be replaced by a
            // file-read from the app-private plugin dir.
            val path = "plugins/${pluginId}/plugin.js"
            context.assets.open(path).bufferedReader().use { it.readText() }
        } catch (e: Exception) {
            loadError = e.message ?: "unknown error"
            null
        }.also { jsSource = it }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(top = scaffoldPadding.calculateTopPadding())
    ) {
        // Top bar
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp)
                .padding(pluginsPaddingX, 0.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                modifier = Modifier
                    .size(28.dp)
                    .clip(RoundedCornerShape(14.dp))
                    .clickable { navController.popBackStack() }
                    .padding(2.dp),
                painter = painterResource(id = R.drawable.icon_back),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
            )
            Box(modifier = Modifier.width(16.dp))
            Text(
                text = pluginDisplayName(pluginId),
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }

        when {
            loadError != null -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "Plugin load failed: $loadError",
                    color = MaterialTheme.colorScheme.error,
                    fontSize = 14.sp,
                )
            }
            jsSource == null -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "Loading…",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 14.sp,
                )
            }
            else -> TurView(
                runtime = EasePluginBridge.runtime(context),
                js = jsSource!!,
                pluginId = pluginId,
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

/** Resolve a plugin's display name from the built-in registry, if known. */
private fun pluginDisplayName(pluginId: String): String {
    return when (pluginId) {
        PluginRepository.PLAYCOUNT_ID -> "Play Counts"
        PluginRepository.TURTEST_ID -> "Tur Test"
        else -> pluginId
    }
}
