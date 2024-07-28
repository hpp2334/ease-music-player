package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.Routes
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.easeIconButtonSizeToDp
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import uniffi.ease_client.playAllMusics
import uniffi.ease_client.prepareImportEntriesInCurrentPlaylist

@Composable
private fun MusicPlayerHeader(
    onRemoveDialogOpen: () -> Unit,
) {
    val navController = LocalNavController.current
    var moreMenuExpanded by remember {
        mutableStateOf(false)
    }

    Row(
        horizontalArrangement = Arrangement.SpaceBetween,
        modifier = Modifier
            .padding(13.dp, 13.dp)
            .fillMaxWidth()
    ) {
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Surface,
            painter = painterResource(id = R.drawable.icon_back),
            onClick = {
                navController.popBackStack()
            }
        )
        Box {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Medium,
                buttonType = EaseIconButtonType.Surface,
                painter = painterResource(id = R.drawable.icon_vertialcal_more),
                onClick = { moreMenuExpanded = true; }
            )
            Box(
                contentAlignment = Alignment.TopEnd,
                modifier = Modifier
                    .offset(20.dp, (20).dp)
            ) {
                EaseContextMenu(
                    expanded = moreMenuExpanded,
                    onDismissRequest = { moreMenuExpanded = false; },
                    items = listOf(
                        EaseContextMenuItem(
                            stringId = R.string.music_player_context_menu_remove,
                            isError = true,
                            onClick = {
                                onRemoveDialogOpen()
                            }
                        ),
                    )
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MusicSlider(
    currentDuration: String,
    currentDurationMS: ULong,
    totalDuration: String,
    totalDurationMS: ULong,
) {
    val durationPercentage = 1.0f * currentDurationMS.toFloat() / totalDurationMS.toFloat()

    Column {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(4.dp)
        ) {
            Row(modifier = Modifier
                .fillMaxSize()
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
            ) {
                Box(modifier = Modifier
                    .fillMaxWidth(durationPercentage)
                    .fillMaxHeight()
                    .background(MaterialTheme.colorScheme.primary)
                )
            }
            // Offset
            Row(modifier = Modifier
                .fillMaxSize()
            ) {
                Box(modifier = Modifier
                    .fillMaxWidth(durationPercentage)
                    .fillMaxHeight()
                )
                Box(modifier = Modifier
                    .size(12.dp)
                    .clip(RoundedCornerShape(999.dp))
                    .background(MaterialTheme.colorScheme.primary))
            }
        }
        Row(
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                text = currentDuration
            )
            Text(
                text = totalDuration
            )
        }
    }
}

@Composable
fun MusicPlayerPage(
    vm: CurrentMusicViewModel
) {
    val state = vm.state.collectAsState().value
    var removeDialogOpen by remember {
        mutableStateOf(false)
    }


    Box(
        modifier = Modifier
            .fillMaxSize()
    ) {
        Column {
            MusicPlayerHeader(
                onRemoveDialogOpen = {
                    removeDialogOpen = true
                },
            )
            Column {
                Text(
                    text = state.title,
                    maxLines = 3,
                )
                MusicSlider(
                    currentDuration = state.currentDuration,
                    currentDurationMS = state.currentDurationMs,
                    totalDuration = state.totalDuration,
                    totalDurationMS = state.currentDurationMs
                )
            }
        }
    }
}