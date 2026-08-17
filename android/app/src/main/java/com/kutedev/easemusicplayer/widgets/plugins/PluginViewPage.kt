package com.kutedev.easemusicplayer.widgets.plugins

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurView
import com.kutedev.easemusicplayer.viewmodels.DashboardVM

/**
 * Standalone plugin-view page (pushed from a Dashboard entry card). Resolves
 * the contribution's **module-source handle** from the plugin scan
 * (`plugin.list` — the Rust side read the file and registered it on the
 * runtime) via [DashboardVM] and renders it in a full-screen [TurView].
 * The plugin JS owns all view logic and data access via the `ease` and
 * `tur:std` modules — the host stays decoupled, and the JS bytes never
 * cross the Kotlin↔Rust boundary.
 *
 * `pluginId` selects the installed plugin dir AND is stamped into the
 * instance's per-instance data slot so `ease:*` bridge fns resolve the
 * calling plugin from Rust; `viewId` selects the contribution entry whose
 * view is loaded.
 */
@Composable
fun PluginViewPage(
    pluginId: String,
    viewId: String,
    scaffoldPadding: PaddingValues,
    dashboardVM: DashboardVM = hiltViewModel(),
) {
    val context = LocalContext.current
    val navController = LocalNavController.current
    val items by dashboardVM.dashboardItems.collectAsState()
    val item = items.find { it.pluginId == pluginId && it.contributionId == viewId }

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
                .padding(24.dp, 0.dp),
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
                text = item?.title ?: pluginId,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }

        when {
            item == null -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "Unknown plugin view: $pluginId / $viewId",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 14.sp,
                )
            }
            item.viewSourceHandle == 0L -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "Plugin view load failed: no source handle",
                    color = MaterialTheme.colorScheme.error,
                    fontSize = 14.sp,
                )
            }
            else -> TurView(
                runtime = EasePluginBridge.runtime(context),
                sourceHandle = item.viewSourceHandle,
                pluginId = pluginId,
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}
