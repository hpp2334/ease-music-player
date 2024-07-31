package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import uniffi.ease_client.setCurrentMusicPositionForPlayerInternal

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
    onChangeMusicPosition: (ms: ULong) -> Unit,
) {
    val handleSize = 12.dp
    val sliderHeight = 4.dp
    val sliderContainerHeight = 16.dp

    val durationRate = (currentDurationMS.toDouble() / totalDurationMS.toDouble()).toFloat()
    var sliderWidth by remember { mutableIntStateOf(0) }
    val sliderWidthDp = with(LocalDensity.current) {
        sliderWidth.toDp()
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(sliderContainerHeight)
        ) {
            Row(modifier = Modifier
                .fillMaxWidth()
                .height(sliderHeight)
                .offset(0.dp, (sliderContainerHeight - sliderHeight) / 2)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .onSizeChanged { size ->
                    sliderWidth = size.width;
                }
            ) {
                Box(modifier = Modifier
                    .fillMaxWidth(durationRate)
                    .fillMaxHeight()
                    .background(MaterialTheme.colorScheme.primary)
                )
            }
            // Offset
            Box(modifier = Modifier
                .offset(-handleSize / 2 + (sliderWidthDp * durationRate), (sliderContainerHeight - handleSize) / 2)
                .size(handleSize)
                .clip(RoundedCornerShape(999.dp))
                .background(MaterialTheme.colorScheme.primary)
                .draggable(rememberDraggableState { delta ->
                    var nextMS =
                        currentDurationMS.toLong() + (delta.toDouble() / sliderWidth.toDouble() * totalDurationMS.toDouble()).toLong()
                    nextMS = nextMS.coerceIn(0L, totalDurationMS.toLong())
                    onChangeMusicPosition(nextMS.toULong())
                }, Orientation.Horizontal)
            )
        }
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            modifier = Modifier
                .fillMaxWidth()
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
                    totalDurationMS = state.currentDurationMs,
                    onChangeMusicPosition = { nextMS ->
                        Bridge.invoke {
                            setCurrentMusicPositionForPlayerInternal(nextMS)
                        }
                    }
                )
            }
        }
    }
}

@Preview(
    widthDp = 400,
    heightDp = 400,
)
@Composable
private fun MusicSliderPreview() {
    fun formatMS(ms: ULong): String {
        var seconds = ms / 1000u
        val minutes = seconds / 60u
        seconds %= 60u

        val m = minutes.toString().padStart(2, '0')
        val s = seconds.toString().padStart(2, '0')

        return "${m}:${s}"
    }

    val totalMS = 120uL * 1000uL
    var currentMS by remember {
        mutableStateOf(0uL)
    }

    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier
            .fillMaxSize()
            .padding(20.dp)
    ) {
        MusicSlider(
            currentDuration = formatMS(currentMS),
            currentDurationMS = currentMS,
            totalDuration = formatMS(totalMS),
            totalDurationMS = totalMS,
            onChangeMusicPosition = {nextMS ->
                currentMS = nextMS
            }
        )
    }
}