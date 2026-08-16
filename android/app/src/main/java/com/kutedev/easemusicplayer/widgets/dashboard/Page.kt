package com.kutedev.easemusicplayer.widgets.dashboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.viewmodel.compose.viewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.viewmodels.DashboardVM
import com.kutedev.easemusicplayer.viewmodels.EditStorageVM
import com.kutedev.easemusicplayer.viewmodels.SleepModeLeftTime
import com.kutedev.easemusicplayer.viewmodels.SleepModeVM
import com.kutedev.easemusicplayer.viewmodels.StoragesVM
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RouteCreateStorage
import com.kutedev.easemusicplayer.core.RouteEditStorage
import com.kutedev.easemusicplayer.core.RoutePluginView
import com.kutedev.easemusicplayer.singleton.DashboardItem
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageHandle
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurView

private val paddingX = 24.dp
private val paddingY = 12.dp

@Composable
private fun Title(title: String) {
    Text(
        text = title,
        color = MaterialTheme.colorScheme.primary,
        fontSize = 14.sp,
    )
}

@Composable
private fun SleepModeBlock(vm: SleepModeVM = hiltViewModel()) {
    val state by vm.state.collectAsState()
    val blockBg = if (state.enabled) {
        MaterialTheme.colorScheme.secondary
    } else {
        MaterialTheme.colorScheme.surfaceVariant
    }
    val tint = if (state.enabled) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }

    var leftTime by remember { mutableStateOf(SleepModeLeftTime(state.expiredMs - System.currentTimeMillis())) }

    LaunchedEffect(state.expiredMs, state.enabled) {
        while (true) {
            leftTime = SleepModeLeftTime(state.expiredMs - System.currentTimeMillis())

            if (!state.enabled) {
                break
            }
            kotlinx.coroutines.delay(1_000)
        }
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(90.dp)
            .padding(paddingX, 0.dp)
            .clip(RoundedCornerShape(16.dp))
            .clickable {
                vm.openModal(leftTime)
            },
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxSize()
                .background(blockBg)
                .padding(32.dp, 24.dp),
        ) {
            Text(
                text = "${leftTime.hour.toString().padStart(2, '0')}:${leftTime.minute.toString().padStart(2, '0')}",
                fontSize = 32.sp,
                color = tint,
            )
            Icon(
                painter = painterResource(id = R.drawable.icon_timelapse),
                contentDescription = null,
                tint = tint,
            )
        }
    }
}

@Composable
private fun ColumnScope.DevicesBlock(
    storageItems: List<Storage>,
    enabledPluginIds: Set<String>,
    editStoragesVM: EditStorageVM = hiltViewModel()
) {
    val navController = LocalNavController.current

    Column(
        modifier = Modifier
            .verticalScroll(rememberScrollState())
            .weight(1f)
            .padding(paddingX, paddingY)
    ) {
        if (storageItems.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(72.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant)
                    .clickable {
                        navController.navigate(RouteCreateStorage())
                    }
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.align(Alignment.Center)
                ) {
                    Icon(
                        modifier = Modifier.size(12.dp),
                        painter = painterResource(id = R.drawable.icon_plus),
                        contentDescription = null
                    )
                    Box(modifier = Modifier.size(4.dp))
                    Text(
                        text = stringResource(id = R.string.dashboard_devices_add),
                        textAlign = TextAlign.Center
                    )
                }
            }
            return
        }
        for (item in storageItems) {
            val handle = item.handle as? StorageHandle.Plugin
            val pluginAlive = handle == null || handle.pluginId.id in enabledPluginIds
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = pluginAlive) {
                        if (pluginAlive) {
                            navController.navigate(RouteEditStorage(item.id.value.toString()))
                        }
                    },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val title = item.alias.ifBlank { item.id.value.toString() }
                val subTitle = when {
                    handle != null && !pluginAlive ->
                        stringResource(id = R.string.plugin_storage_removed)
                    handle != null -> handle.pluginStorageId.id.substringBefore(':')
                    else -> ""
                }

                Box(modifier = Modifier.height(48.dp))
                Icon(
                    modifier = Modifier.size(32.dp),
                    painter = painterResource(id = R.drawable.icon_cloud),
                    contentDescription = null
                )
                Box(
                    modifier = Modifier
                        .width(20.dp)
                )
                Column {
                    Text(
                        text = title,
                        fontSize = 14.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (subTitle.isNotBlank()) {
                        Text(
                            text = subTitle,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontSize = 12.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}

/**
 * One plugin dashboard **entry card**: the contribution's title + plugin
 * id. Not the view itself — tapping pushes [PluginViewPage], the
 * standalone full-screen page that renders the plugin's view JS.
 */
@Composable
private fun DashboardCard(
    item: DashboardItem,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(paddingX, 0.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .clickable { onClick() }
            .padding(20.dp, 14.dp),
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
            Text(
                text = item.title,
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = item.pluginId,
                fontSize = 11.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Icon(
            modifier = Modifier.size(16.dp),
            painter = painterResource(id = R.drawable.icon_collapse),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
fun DashboardSubpage(
    storageVM: StoragesVM = hiltViewModel(),
    editStoragesVM: EditStorageVM = hiltViewModel(),
    dashboardVM: DashboardVM = hiltViewModel(),
) {
    val navController = LocalNavController.current
    val storages by storageVM.storages.collectAsState()
    val storageItems = storages.filter { v -> v.handle !is StorageHandle.Local }
    val enabledPlugins by dashboardVM.enabledPlugins.collectAsState()
    val dashboardItems by dashboardVM.dashboardItems.collectAsState()
    val enabledPluginIds = enabledPlugins.map { it.id }.toSet()

    LaunchedEffect(Unit) {
        storageVM.reload()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        Box(modifier = Modifier.height(48.dp))
        Row(
            modifier = Modifier
                .padding(paddingX, 4.dp)
                .fillMaxWidth(),
        ) {
            Title(title = stringResource(id = R.string.dashboard_sleep_mode))
        }
        SleepModeBlock()
        Box(modifier = Modifier.height(48.dp))
        Row(
            modifier = Modifier
                .padding(paddingX, 4.dp)
                .fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Title(title = stringResource(id = R.string.dashboard_devices))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (storageItems.isNotEmpty()) {
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Small,
                        buttonType = EaseIconButtonType.Primary,
                        painter = painterResource(id = R.drawable.icon_plus),
                        onClick = {
                            navController.navigate(RouteCreateStorage())
                        }
                    )
                }
            }
        }
        DevicesBlock(storageItems, enabledPluginIds)
        if (dashboardItems.isNotEmpty()) {
            Box(modifier = Modifier.height(48.dp))
            Row(
                modifier = Modifier
                    .padding(paddingX, 4.dp)
                    .fillMaxWidth(),
            ) {
                Title(title = stringResource(id = R.string.dashboard_plugins))
            }
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (item in dashboardItems) {
                    DashboardCard(item) {
                        navController.navigate(
                            RoutePluginView(item.pluginId, item.contributionId)
                        )
                    }
                }
            }
            Box(modifier = Modifier.height(24.dp))
        }
    }
}
