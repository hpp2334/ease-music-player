package com.kutedev.easemusicplayer.widgets.dashboard

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel

private val paddingX = 24.dp

@Composable
private fun Title(title: String) {
    Text(
        text = title,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(paddingX, 0.dp)
    )
}

@Composable
private fun SleepModeBlock(timeToPauseViewModel: TimeToPauseViewModel) {
    val state = timeToPauseViewModel.state.collectAsState().value
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

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(90.dp)
            .padding(paddingX, 0.dp)
            .clip(RoundedCornerShape(16.dp)),
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
private fun DevicesBlock() {

}

@Composable
fun DashboardSubpage(
    timeToPauseVM: TimeToPauseViewModel,
    storageListVM: StorageListViewModel,
) {
    val storageState = storageListVM.state.collectAsState().value

    Column(
        modifier = Modifier.fillMaxSize()
    ) {
        Box(modifier = Modifier.height(48.dp))
        Title(title = stringResource(id = R.string.dashboard_sleep_mode))
        SleepModeBlock(timeToPauseViewModel = timeToPauseVM)
        Box(modifier = Modifier.height(48.dp))
        Row {
            Title(title = stringResource(id = R.string.dashboard_devices))
            if (storageState.items.isEmpty()) {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Small,
                    buttonType = EaseIconButtonType.Primary,
                    painter = painterResource(id = R.drawable.icon_plus),
                    onClick = {}
                )
            }
        }
        DevicesBlock()
    }
}