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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.singleton.PluginSource
import com.kutedev.easemusicplayer.viewmodels.AvailableListState
import com.kutedev.easemusicplayer.viewmodels.AvailablePluginRow
import com.kutedev.easemusicplayer.viewmodels.AvailablePluginsVM

private val pluginsPaddingX = 24.dp

/**
 * The "get plugins" page: pick a registry source (jsDelivr / fastly /
 * gcore / GitHub Raw presets + verified custom sources), browse its
 * plugin list (cached offline), and install / update entries.
 */
@Composable
fun AvailablePluginsPage(
    scaffoldPadding: PaddingValues,
    vm: AvailablePluginsVM = hiltViewModel(),
) {
    val navController = LocalNavController.current
    val sources by vm.sources.collectAsState()
    val selectedSourceUrl by vm.selectedSourceUrl.collectAsState()
    val listState by vm.listState.collectAsState()
    val busyIds by vm.busyIds.collectAsState()

    var sourceDialogOpen by remember { mutableStateOf(false) }
    var customDialogOpen by remember { mutableStateOf(false) }

    val selectedSource = sources.firstOrNull { it.url == selectedSourceUrl }

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
                text = stringResource(id = R.string.plugin_available_title),
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Box(modifier = Modifier.weight(1f))
            EaseIconButton(
                sizeType = EaseIconButtonSize.Small,
                buttonType = EaseIconButtonType.Default,
                painter = painterResource(id = R.drawable.icon_vertialcal_more),
                onClick = { vm.retry() }
            )
        }

        // Source selector chip
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(pluginsPaddingX, 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(id = R.string.plugin_source_label),
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Box(modifier = Modifier.width(8.dp))
            Row(
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .clickable { sourceDialogOpen = true }
                    .padding(12.dp, 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = selectedSource?.label ?: selectedSourceUrl,
                    fontSize = 13.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Box(modifier = Modifier.weight(1f))
                Text(
                    text = "▾",
                    fontSize = 13.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // List
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(pluginsPaddingX, 8.dp),
        ) {
            when (val state = listState) {
                is AvailableListState.Loading -> Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(0.dp, 32.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(24.dp))
                }
                is AvailableListState.Failed -> Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(0.dp, 24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = stringResource(id = R.string.plugin_registry_fail),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontSize = 14.sp,
                    )
                    EaseTextButton(
                        text = stringResource(id = R.string.plugin_retry),
                        type = EaseTextButtonType.Primary,
                        size = EaseTextButtonSize.Medium,
                        onClick = { vm.retry() },
                    )
                }
                is AvailableListState.Ready -> {
                    if (state.fromCache) {
                        Text(
                            text = stringResource(id = R.string.plugin_registry_cached),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontSize = 11.sp,
                        )
                        Box(modifier = Modifier.height(8.dp))
                    }
                    if (state.rows.isEmpty()) {
                        Text(
                            text = stringResource(id = R.string.plugin_available_empty),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontSize = 14.sp,
                        )
                    }
                    for (row in state.rows) {
                        AvailablePluginRowItem(
                            row = row,
                            busy = row.entry.id in busyIds,
                            vm = vm,
                        )
                    }
                }
            }
            Box(modifier = Modifier.height(24.dp))
        }
    }

    if (sourceDialogOpen) {
        SourceDialog(
            sources = sources,
            selectedUrl = selectedSourceUrl,
            onSelect = {
                vm.selectSource(it)
                sourceDialogOpen = false
            },
            onRemoveCustom = { vm.removeCustomSource(it) },
            onAddCustom = {
                sourceDialogOpen = false
                customDialogOpen = true
            },
            onDismiss = { sourceDialogOpen = false },
        )
    }
    if (customDialogOpen) {
        CustomSourceDialog(
            onVerify = { url ->
                vm.addCustomSource(url) { ok ->
                    if (ok) customDialogOpen = false
                }
            },
            onDismiss = { customDialogOpen = false },
        )
    }
}

@Composable
private fun SourceDialog(
    sources: List<PluginSource>,
    selectedUrl: String,
    onSelect: (String) -> Unit,
    onRemoveCustom: (String) -> Unit,
    onAddCustom: () -> Unit,
    onDismiss: () -> Unit,
) {
    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(20.dp, 16.dp),
        ) {
            Text(
                text = stringResource(id = R.string.plugin_source_label),
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Box(modifier = Modifier.height(8.dp))
            for (source in sources) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .clickable { onSelect(source.url) }
                        .padding(8.dp, 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = if (source.url == selectedUrl) "● " else "○ ",
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = source.label,
                            fontSize = 14.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = source.url,
                            fontSize = 10.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (!source.preset) {
                        EaseIconButton(
                            sizeType = EaseIconButtonSize.Small,
                            buttonType = EaseIconButtonType.Error,
                            painter = painterResource(id = R.drawable.icon_deleteseep),
                            onClick = { onRemoveCustom(source.url) },
                        )
                    }
                }
            }
            Box(modifier = Modifier.height(4.dp))
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .clickable { onAddCustom() }
                    .padding(8.dp, 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = stringResource(id = R.string.plugin_source_custom),
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.primary,
                )
            }
        }
    }
}

@Composable
private fun CustomSourceDialog(
    onVerify: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var url by remember { mutableStateOf("https://") }
    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(20.dp, 16.dp),
        ) {
            Text(
                text = stringResource(id = R.string.plugin_source_custom),
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Box(modifier = Modifier.height(4.dp))
            Text(
                text = stringResource(id = R.string.plugin_source_custom_desc),
                fontSize = 11.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Box(modifier = Modifier.height(8.dp))
            OutlinedTextField(
                value = url,
                onValueChange = { url = it },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            Box(modifier = Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                EaseTextButton(
                    text = stringResource(id = R.string.time_to_pause_cancel),
                    type = EaseTextButtonType.Default,
                    size = EaseTextButtonSize.Medium,
                    onClick = onDismiss,
                )
                EaseTextButton(
                    text = stringResource(id = R.string.plugin_source_verify),
                    type = EaseTextButtonType.Primary,
                    size = EaseTextButtonSize.Medium,
                    onClick = { onVerify(url) },
                )
            }
        }
    }
}

@Composable
private fun AvailablePluginRowItem(
    row: AvailablePluginRow,
    busy: Boolean,
    vm: AvailablePluginsVM,
) {
    val hasUpdate = row.installedVersion != null &&
        vm.compareVersions(row.entry.version, row.installedVersion) > 0
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(0.dp, 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            modifier = Modifier.size(32.dp),
            painter = painterResource(id = R.drawable.icon_extension),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.primary,
        )
        Box(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = row.entry.name,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Box(modifier = Modifier.width(8.dp))
                Text(
                    text = row.entry.version,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 12.sp,
                )
                if (hasUpdate) {
                    Box(modifier = Modifier.width(6.dp))
                    Text(
                        text = stringResource(id = R.string.plugin_update_available),
                        color = MaterialTheme.colorScheme.primary,
                        fontSize = 11.sp,
                    )
                }
            }
            if (row.entry.description.isNotBlank()) {
                Text(
                    text = row.entry.description,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Box(modifier = Modifier.width(8.dp))
        if (busy) {
            CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
        } else if (row.installedVersion == null) {
            EaseTextButton(
                text = stringResource(id = R.string.plugin_install),
                type = EaseTextButtonType.PrimaryVariant,
                size = EaseTextButtonSize.Small,
                onClick = { vm.install(row.entry) },
            )
        } else if (hasUpdate) {
            EaseTextButton(
                text = stringResource(id = R.string.plugin_update),
                type = EaseTextButtonType.PrimaryVariant,
                size = EaseTextButtonSize.Small,
                onClick = { vm.install(row.entry) },
            )
        } else {
            Text(
                text = stringResource(id = R.string.plugin_installed_tag),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 12.sp,
            )
        }
    }
}
