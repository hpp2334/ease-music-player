package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonColors
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.dashboard.TimeToPauseModal
import uniffi.ease_client.MusicControlAction
import uniffi.ease_client.MusicControlWidget
import uniffi.ease_client.VCurrentMusicState
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.PlayMode

@Composable
private fun MusicPlayerHeader(
    onRemoveDialogOpen: () -> Unit,
) {
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
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = R.drawable.icon_back),
            onClick = {
                Bridge.popRoute()
            }
        )
        Box {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Medium,
                buttonType = EaseIconButtonType.Default,
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

    val durationRate = if (totalDurationMS == 0UL) { 0f } else { (currentDurationMS.toDouble() / totalDurationMS.toDouble()).toFloat() };
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
                .onSizeChanged { size ->
                    sliderWidth = size.width;
                }
                .pointerInput(sliderWidth, totalDurationMS) {
                    awaitPointerEventScope {
                        while (true) {
                            val event = awaitPointerEvent()
                            val position = event.changes.first().position

                            var nextMS =
                                (position.x / sliderWidth.toDouble() * totalDurationMS.toDouble()).toLong()
                            nextMS = nextMS.coerceIn(0L, totalDurationMS.toLong())
                            onChangeMusicPosition(nextMS.toULong())
                        }
                    }
                }
        ) {
            Row(modifier = Modifier
                .fillMaxWidth()
                .height(sliderHeight)
                .offset(0.dp, (sliderContainerHeight - sliderHeight) / 2)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
            ) {
                Box(modifier = Modifier
                    .fillMaxWidth(durationRate)
                    .fillMaxHeight()
                    .background(MaterialTheme.colorScheme.primary)
                )
            }
            Box(modifier = Modifier
                .offset(
                    -handleSize / 2 + (sliderWidthDp * durationRate),
                    (sliderContainerHeight - handleSize) / 2
                )
                .size(handleSize)
                .clip(RoundedCornerShape(999.dp))
                .background(MaterialTheme.colorScheme.primary)
            )
        }
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            modifier = Modifier
                .fillMaxWidth()
        ) {
            Text(
                text = currentDuration,
                fontSize = 10.sp
            )
            Text(
                text = totalDuration,
                fontSize = 10.sp
            )
        }
    }
}

@Composable
private fun MusicPlayerCover(
    onLeft: () -> Unit,
    onRight: () -> Unit,
    cover: String,
    prevCover: String,
    nextCover: String
) {
    @Composable
    fun CoverImage(url: String) {

        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .fillMaxSize()
        ) {
            Box(
                modifier = Modifier
                    .padding(horizontal = 37.dp)
                    .shadow(
                        elevation = 8.dp,
                        shape = RoundedCornerShape(20.dp)
                    )
                    .clip(RoundedCornerShape(20.dp))
            ) {
                if (url.isEmpty()) {
                    Image(
                        painter = painterResource(id = R.drawable.cover_default_image),
                        contentDescription = null
                    )
                } else {
                    AsyncImage(
                        model = url,
                        contentDescription = null,
                    )
                }
            }
        }
    }

    CoverImage(url = cover)
}

@Composable
private fun MusicPanel(
    state: VCurrentMusicState,
    timeToPauseVM: TimeToPauseViewModel
) {
    var isOpen by remember { mutableStateOf(false) }
    val modeDrawable = when (state.playMode) {
        PlayMode.SINGLE -> R.drawable.icon_mode_one
        PlayMode.SINGLE_LOOP -> R.drawable.icon_mode_repeatone
        PlayMode.LIST -> R.drawable.icon_mode_list
        PlayMode.LIST_LOOP -> R.drawable.icon_mode_repeat
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = if (isOpen) { EaseIconButtonType.Primary } else { EaseIconButtonType.Default },
            overrideColors = EaseIconButtonColors(
                iconTint = if (isOpen) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onSurface
                },
                buttonBg = Color.Transparent,
            ),
            painter = painterResource(id = R.drawable.icon_timelapse),
            onClick = {
                isOpen = true;
            }
        )
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = R.drawable.icon_play_previous),
            disabled = !state.canPlayPrevious,
            onClick = {
                Bridge.dispatchClick(MusicControlWidget.PLAY_PREVIOUS)
            }
        )
        if (!state.playing) {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Large,
                buttonType = EaseIconButtonType.Primary,
                painter = painterResource(id = R.drawable.icon_play),
                onClick = {
                    Bridge.dispatchClick(MusicControlWidget.PLAY)
                }
            )
        }
        if (state.playing) {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Large,
                buttonType = EaseIconButtonType.Primary,
                painter = painterResource(id = R.drawable.icon_pause),
                onClick = {
                    Bridge.dispatchClick(MusicControlWidget.PAUSE)
                }
            )
        }
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = R.drawable.icon_play_next),
            disabled = !state.canPlayNext,
            onClick = {
                Bridge.dispatchClick(MusicControlWidget.PLAY_NEXT)
            }
        )
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = modeDrawable),
            onClick = {
                Bridge.dispatchClick(MusicControlWidget.PLAYMODE)
            }
        )
    }
}

@Composable
fun MusicPlayerPage(
    vm: CurrentMusicViewModel,
    timeToPauseVM: TimeToPauseViewModel
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
            Column(
                modifier = Modifier
                    .weight(1.0F)
            ) {
                MusicPlayerCover(
                    onLeft = {
                        Bridge.dispatchClick(MusicControlWidget.PLAY_PREVIOUS)
                    },
                    onRight = {
                        Bridge.dispatchClick(MusicControlWidget.PLAY_NEXT)
                    },
                    cover = state.cover,
                    prevCover = state.previousCover,
                    nextCover = state.nextCover,
                )
            }
            Column(
                modifier = Modifier.padding(36.dp, 10.dp)
            ) {
                Text(
                    text = state.title,
                    maxLines = 3,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontSize = 20.sp,
                    modifier = Modifier.padding(0.dp, 10.dp)
                )
                MusicSlider(
                    currentDuration = state.currentDuration,
                    currentDurationMS = state.currentDurationMs,
                    totalDuration = state.totalDuration,
                    totalDurationMS = state.totalDurationMs,
                    onChangeMusicPosition = { nextMS ->
                        Bridge.dispatchAction(ViewAction.MusicControl(MusicControlAction.Seek(nextMS)))
                    }
                )
            }
            Row(
                modifier = Modifier
                    .align(Alignment.CenterHorizontally)
                    .padding(0.dp, 48.dp)
            ) {
                MusicPanel(
                    state = state,
                    timeToPauseVM = timeToPauseVM,
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
        mutableStateOf(60uL * 1000uL)
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


@Preview(
    widthDp = 400,
    heightDp = 800,
)
@Composable
private fun MusicCoverPreview() {
    Column(
        modifier = Modifier
            .fillMaxSize()
    ) {
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .background(Color.Blue))
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier.weight(1f)
        ) {
            MusicPlayerCover(
                onLeft = {
                    println("onLeft")
                },
                onRight = {
                    println("onRight")
                },
                cover = "",
                prevCover = "",
                nextCover = ""
            )
        }
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .background(Color.Blue))
    }
}
