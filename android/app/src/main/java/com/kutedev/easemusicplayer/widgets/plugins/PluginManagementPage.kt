package com.kutedev.easemusicplayer.widgets.plugins

import androidx.compose.animation.AnimatedVisibility
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RoutePluginAvailable
import com.kutedev.easemusicplayer.singleton.PluginManifest
import com.kutedev.easemusicplayer.singleton.PluginManager
import com.kutedev.easemusicplayer.singleton.resolve
import com.kutedev.easemusicplayer.viewmodels.PluginManagementVM

private val pluginsPaddingX = 24.dp

/** Pending uninstall (dialog open until confirmed). */
private data class PendingUninstall(
    val plugin: PluginManifest,
    val storageCount: Int,
)

/**
 * Central plugin management: the installed list (enable switch +
 * uninstall), the SAF install-from-zip entry, and the top-right button
 * pushing to [AvailablePluginsPage] (network registry).
 */
@Composable
fun PluginManagementPage(
    scaffoldPadding: PaddingValues,
    pluginsVM: PluginManagementVM = hiltViewModel(),
) {
    val navController = LocalNavController.current
    val plugins by pluginsVM.installedPlugins.collectAsState()
    val storages by pluginsVM.storages.collectAsState()
    val busyIds by pluginsVM.busyIds.collectAsState()

    var pendingUninstall by remember { mutableStateOf<PendingUninstall?>(null) }

    val zipPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            pluginsVM.installFromUri(uri)
        }
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
            IconButton(
                modifier = Modifier.size(40.dp),
                onClick = { navController.popBackStack() },
            ) {
                Icon(
                    modifier = Modifier.size(24.dp),
                    painter = painterResource(id = R.drawable.icon_back),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurface,
                )
            }
            Box(modifier = Modifier.width(12.dp))
            Text(
                text = stringResource(id = R.string.plugin_management_title),
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Box(modifier = Modifier.weight(1f))
            IconButton(
                modifier = Modifier.size(40.dp),
                onClick = {
                    navController.navigate(RoutePluginAvailable())
                }
            ) {
                Icon(
                    modifier = Modifier.size(24.dp),
                    painter = painterResource(id = R.drawable.icon_download),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurface,
                )
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(pluginsPaddingX, 4.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier
                        .size(3.dp, 16.dp)
                        .clip(RoundedCornerShape(1.5.dp))
                        .background(BadgeGreen)
                )
                Box(modifier = Modifier.width(8.dp))
                Text(
                    text = stringResource(id = R.string.plugin_installed_section),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 13.sp,
                )
            }
            Box(modifier = Modifier.height(12.dp))
            if (plugins.isEmpty()) {
                Text(
                    text = stringResource(id = R.string.plugin_installed_empty),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 14.sp,
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                for (plugin in plugins) {
                    InstalledPluginRow(
                        plugin = plugin,
                        busy = plugin.id in busyIds,
                        storageCount = pluginsVM.storageCountFor(plugin.id, storages),
                        onToggle = { pluginsVM.setEnabled(plugin.id, it) },
                        onUninstall = {
                            pendingUninstall = PendingUninstall(
                                plugin,
                                pluginsVM.storageCountFor(plugin.id, storages),
                            )
                        },
                    )
                }
            }
            Box(modifier = Modifier.height(16.dp))
            val dashColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .drawBehind {
                        drawRoundRect(
                            color = dashColor,
                            cornerRadius = CornerRadius(16.dp.toPx()),
                            style = Stroke(
                                width = 1.5.dp.toPx(),
                                pathEffect = PathEffect.dashPathEffect(
                                    floatArrayOf(8.dp.toPx(), 6.dp.toPx())
                                ),
                            ),
                        )
                    }
                    .clip(RoundedCornerShape(16.dp))
                    .clickable { zipPicker.launch(arrayOf("application/zip", "application/octet-stream")) }
                    .padding(16.dp, 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Center,
            ) {
                Icon(
                    modifier = Modifier.size(16.dp),
                    painter = painterResource(id = R.drawable.icon_file),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Box(modifier = Modifier.width(8.dp))
                Text(
                    text = stringResource(id = R.string.plugin_install_from_zip),
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Box(modifier = Modifier.height(24.dp))
        }
    }

    pendingUninstall?.let { pending ->
        ConfirmDialog(
            open = true,
            onConfirm = {
                pluginsVM.uninstall(pending.plugin.id)
                pendingUninstall = null
            },
            onCancel = { pendingUninstall = null },
        ) {
            Text(
                text = stringResource(id = R.string.plugin_uninstall_confirm)
                    .replace("E_NAME", pending.plugin.name.resolve()),
                fontSize = 14.sp,
            )
            if (pending.storageCount > 0) {
                Text(
                    text = stringResource(id = R.string.plugin_uninstall_storage_warning)
                        .replace("E_COUNT", pending.storageCount.toString()),
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

@Composable
private fun InstalledPluginRow(
    plugin: PluginManifest,
    busy: Boolean,
    storageCount: Int,
    onToggle: (Boolean) -> Unit,
    onUninstall: () -> Unit,
) {
    val dim = if (plugin.enabled) 1f else 0.45f
    val accent = pluginAccent(plugin.id)
    var expanded by remember { mutableStateOf(false) }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .clickable { expanded = !expanded }
            .padding(14.dp, 12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .alpha(dim)
                    .clip(RoundedCornerShape(12.dp))
                    .background(accent.copy(alpha = 0.15f)),
                contentAlignment = Alignment.Center,
            ) {
                if (busy) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(22.dp),
                        strokeWidth = 2.dp,
                    )
                } else {
                    Icon(
                        modifier = Modifier.size(24.dp),
                        painter = painterResource(id = R.drawable.icon_extension),
                        contentDescription = null,
                        tint = accent,
                    )
                }
            }
            Box(modifier = Modifier.width(14.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = plugin.name.resolve(),
                        fontSize = 16.sp,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.alpha(dim),
                    )
                    Box(modifier = Modifier.width(8.dp))
                    Text(
                        text = plugin.version,
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.alpha(dim),
                    )
                    if (!plugin.enabled) {
                        Box(modifier = Modifier.width(8.dp))
                        Text(
                            text = stringResource(id = R.string.plugin_disabled_tag),
                            fontSize = 12.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                val description = plugin.description.resolve()
                if (description.isNotBlank()) {
                    Box(modifier = Modifier.height(2.dp))
                    Text(
                        text = description,
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.alpha(dim),
                    )
                }
            }
            Icon(
                modifier = Modifier.size(16.dp),
                painter = painterResource(id = if (expanded) R.drawable.icon_collapse else R.drawable.icon_forward),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        AnimatedVisibility(visible = expanded) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 12.dp),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = stringResource(
                        id = if (plugin.enabled) R.string.plugin_disable_action else R.string.plugin_enable_action
                    ),
                    fontSize = 13.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Box(modifier = Modifier.width(8.dp))
                IconButton(
                    modifier = Modifier.size(40.dp),
                    onClick = { onToggle(!plugin.enabled) },
                ) {
                    Icon(
                        modifier = Modifier.size(20.dp),
                        painter = painterResource(id = if (plugin.enabled) R.drawable.icon_stop else R.drawable.icon_play),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Box(modifier = Modifier.width(12.dp))
                Text(
                    text = stringResource(id = R.string.plugin_uninstall_action),
                    fontSize = 13.sp,
                    color = MaterialTheme.colorScheme.error,
                )
                Box(modifier = Modifier.width(8.dp))
                IconButton(
                    modifier = Modifier.size(40.dp),
                    onClick = onUninstall,
                ) {
                    Icon(
                        modifier = Modifier.size(20.dp),
                        painter = painterResource(id = R.drawable.icon_deleteseep),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.error,
                    )
                }
            }
        }
    }
}

private val BadgeGreen = Color(0xFF34C759)

internal fun pluginAccent(id: String): Color {
    val mixed = id.hashCode() * -0x61C88647 // golden-ratio scramble for hue spread
    val hue = ((mixed.toLong() and 0xFFFFFFFFL) % 360L).toFloat()
    return Color.hsl(hue = hue, saturation = 0.45f, lightness = 0.55f)
}
