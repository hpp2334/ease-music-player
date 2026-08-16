package com.kutedev.easemusicplayer.widgets.plugins

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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RoutePluginAvailable
import com.kutedev.easemusicplayer.singleton.PluginManifest
import com.kutedev.easemusicplayer.singleton.PluginManager
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
                text = stringResource(id = R.string.plugin_management_title),
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Box(modifier = Modifier.weight(1f))
            EaseIconButton(
                sizeType = EaseIconButtonSize.Small,
                buttonType = EaseIconButtonType.Default,
                painter = painterResource(id = R.drawable.icon_download),
                onClick = {
                    navController.navigate(RoutePluginAvailable())
                }
            )
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(pluginsPaddingX, 4.dp),
        ) {
            Text(
                text = stringResource(id = R.string.plugin_installed_section),
                color = MaterialTheme.colorScheme.primary,
                fontSize = 14.sp,
            )
            Box(modifier = Modifier.height(8.dp))
            if (plugins.isEmpty()) {
                Text(
                    text = stringResource(id = R.string.plugin_installed_empty),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 14.sp,
                )
            }
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
            Box(modifier = Modifier.height(16.dp))
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(16.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .clickable { zipPicker.launch(arrayOf("application/zip", "application/octet-stream")) }
                    .padding(20.dp, 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Center,
            ) {
                Icon(
                    modifier = Modifier.size(16.dp),
                    painter = painterResource(id = R.drawable.icon_file),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurface,
                )
                Box(modifier = Modifier.width(8.dp))
                Text(
                    text = stringResource(id = R.string.plugin_install_from_zip),
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.onSurface,
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
                    .replace("E_NAME", pending.plugin.name),
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
    val dim = if (plugin.enabled) 1f else 0.4f
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .alpha(dim)
            .padding(0.dp, 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (busy) {
            CircularProgressIndicator(
                modifier = Modifier.size(24.dp),
                strokeWidth = 2.dp,
            )
        } else {
            Icon(
                modifier = Modifier.size(32.dp),
                painter = painterResource(id = R.drawable.icon_extension),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
            )
        }
        Box(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = plugin.name,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Box(modifier = Modifier.width(8.dp))
                if (plugin.id == PluginManager.BUNDLED_PLUGIN_ID) {
                    Text(
                        text = stringResource(id = R.string.plugin_bundled_tag),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontSize = 11.sp,
                    )
                }
            }
            if (plugin.description.isNotBlank()) {
                Text(
                    text = plugin.description,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                text = "${plugin.id} · ${plugin.version}",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 11.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Switch(
            checked = plugin.enabled,
            onCheckedChange = onToggle,
            colors = SwitchDefaults.colors(
                checkedTrackColor = MaterialTheme.colorScheme.primary,
            ),
        )
        Box(modifier = Modifier.width(4.dp))
        EaseIconButton(
            sizeType = EaseIconButtonSize.Small,
            buttonType = EaseIconButtonType.Error,
            painter = painterResource(id = R.drawable.icon_deleteseep),
            onClick = onUninstall,
        )
    }
}
