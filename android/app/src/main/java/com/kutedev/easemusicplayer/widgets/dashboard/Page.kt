package com.kutedev.easemusicplayer.widgets.dashboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigation.NavHostController
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.Routes
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import uniffi.ease_client.StorageListWidget
import uniffi.ease_client.VStorageListItem
import uniffi.ease_client_shared.StorageId
import uniffi.ease_client_shared.StorageType

private val paddingX = 24.dp

private fun toEditStorage(navController: NavHostController, arg: StorageId?) {
    if (arg != null) {
        Bridge.dispatchClick(StorageListWidget.Item(arg))
    } else {
        Bridge.dispatchClick(StorageListWidget.Create)
    }
    navController.navigate(Routes.ADD_DEVICES)
}

@Composable
private fun Title(title: String) {
    Text(
        text = title,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun SleepModeBlock(vm: TimeToPauseViewModel) {
    var isOpen by remember { mutableStateOf(false) }
    val state = vm.state.collectAsState().value
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

    TimeToPauseModal(
        vm = vm,
        isOpen = isOpen,
        onClose = {
            isOpen = false;
        }
    )
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(90.dp)
            .padding(paddingX, 0.dp)
            .clip(RoundedCornerShape(16.dp))
            .clickable {
                isOpen = true;
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
                text = "${state.leftHour.toString().padStart(2, '0')}:${state.leftMinute.toString().padStart(2, '0')}",
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
private fun DevicesBlock(storageItems: List<VStorageListItem>) {
    val navController = LocalNavController.current

    Column(
        modifier = Modifier
            .padding(paddingX, 0.dp)
    ) {
        if (storageItems.isEmpty()) {
            Box(modifier = Modifier
                .fillMaxWidth()
                .height(72.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .clickable {
                    toEditStorage(navController, null)
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
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(0.dp, 4.dp)
                    .clickable {
                        toEditStorage(navController, item.storageId)
                    },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    modifier = Modifier.size(32.dp),
                    painter = painterResource(id = R.drawable.icon_cloud),
                    contentDescription = null
                )
                Box(modifier = Modifier
                    .width(20.dp)
                )
                Column {
                    Text(
                        text = item.name,
                        fontSize = 14.sp,
                    )
                    Text(
                        text = item.subTitle,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontSize = 12.sp,
                    )
                }
            }
        }
    }
}

@Composable
fun DashboardSubpage(
    timeToPauseVM: TimeToPauseViewModel,
    storageListVM: StorageListViewModel,
) {
    val storageState = storageListVM.state.collectAsState().value
    val storageItems = storageState.items.filter { v -> v.typ != StorageType.LOCAL }
    val navController = LocalNavController.current

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        Box(modifier = Modifier.height(48.dp))
        Row(
            modifier = Modifier
                .padding(paddingX, 0.dp)
                .fillMaxWidth(),
        ) {
            Title(title = stringResource(id = R.string.dashboard_sleep_mode))
        }
        SleepModeBlock(vm = timeToPauseVM)
        Box(modifier = Modifier.height(48.dp))
        Row(
            modifier = Modifier
                .padding(paddingX, 0.dp)
                .fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Title(title = stringResource(id = R.string.dashboard_devices))
            if (storageItems.isNotEmpty()) {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Small,
                    buttonType = EaseIconButtonType.Primary,
                    painter = painterResource(id = R.drawable.icon_plus),
                    onClick = {
                        toEditStorage(navController, null)
                    }
                )
            }
        }
        DevicesBlock(storageItems)
    }
}