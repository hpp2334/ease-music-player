package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
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
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonColors
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.MusicCover
import com.kutedev.easemusicplayer.components.customAnchoredDraggable
import com.kutedev.easemusicplayer.components.dropShadow
import com.kutedev.easemusicplayer.components.rememberCustomAnchoredDraggableState
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import uniffi.ease_client.MusicControlAction
import uniffi.ease_client.MusicControlWidget
import uniffi.ease_client.MusicDetailWidget
import uniffi.ease_client.VCurrentMusicState
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.PlayMode
import kotlin.math.absoluteValue
import kotlin.math.sign

@Composable
private fun MusicPlayerHeader(
    onRemoveClick: () -> Unit,
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
                                onRemoveClick()
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
    _currentDurationMS: ULong,
    totalDuration: String,
    totalDurationMS: ULong,
    onChangeMusicPosition: (ms: ULong) -> Unit,
) {
    val handleSize = 12.dp
    val sliderHeight = 4.dp
    val sliderContainerHeight = 16.dp

    var isDragging by remember { mutableStateOf(false) }
    var draggingCurrentDurationMS by remember { mutableStateOf(_currentDurationMS) }
    val currentDurationMS = if (isDragging) {
        draggingCurrentDurationMS
    } else {
        _currentDurationMS
    }

    val durationRate = if (totalDurationMS == 0UL) { 0f } else { (currentDurationMS.toDouble() / totalDurationMS.toDouble()).toFloat() };
    var sliderWidth by remember { mutableIntStateOf(0) }
    val sliderWidthDp = with(LocalDensity.current) {
        sliderWidth.toDp()
    }

    val draggableState = rememberDraggableState { deltaPx ->
        val delta = (deltaPx.toDouble() / sliderWidth.toDouble() * totalDurationMS.toDouble()).toLong()
        var nextMS = draggingCurrentDurationMS.toLong() + delta
        nextMS = nextMS.coerceIn(0L, totalDurationMS.toLong())

        draggingCurrentDurationMS = nextMS.toULong()
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
                .pointerInput(Unit) {
                    detectTapGestures { offset ->
                        var nextMS = (offset.x.toDouble() / sliderWidth.toDouble() * totalDurationMS.toDouble()).toLong()
                        nextMS = nextMS.coerceIn(0L, totalDurationMS.toLong())
                        onChangeMusicPosition(nextMS.toULong())
                    }
                }
                .draggable(
                    state = draggableState,
                    orientation = Orientation.Horizontal,
                    onDragStarted = {
                        isDragging = true
                        draggingCurrentDurationMS = _currentDurationMS
                    },
                    onDragStopped = {
                        isDragging = false
                        onChangeMusicPosition(draggingCurrentDurationMS)
                    }
                )
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
private fun CoverImage(url: String) {
    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier
            .fillMaxSize()
    ) {
        MusicCover(
            modifier = Modifier
                .dropShadow(
                    color = MaterialTheme.colorScheme.surfaceVariant,
                    offsetX = 0.dp,
                    offsetY = 0.dp,
                    blurRadius = 16.dp
                )
                .clip(RoundedCornerShape(20.dp))
                .size(300.dp),
            coverUrl = url,
        )
    }
}

@Composable
private fun MusicPlayerBody(
    onPrev: () -> Unit,
    onNext: () -> Unit,
    cover: String,
    prevCover: String,
    nextCover: String,
    canPrev: Boolean,
    canNext: Boolean,
) {
    val density = LocalDensity.current
    val anchoredDraggableState = rememberCustomAnchoredDraggableState(
        initialValue = 0f,
        animationSpec = tween(
            durationMillis = 300,
            easing = LinearOutSlowInEasing
        ),
        anchors = mapOf(0f to "DEFAULT"),
    )
    val deltaDp = with(density) {
        anchoredDraggableState.value.toDp()
    }
    var widgetWidth by remember { mutableIntStateOf(0) }
    val widgetWidthDp = with(LocalDensity.current) {
        widgetWidth.toDp()
    }

    var dragStartX by remember { mutableFloatStateOf(0f) }

    fun updateAnchored() {
        val anchors =listOfNotNull(
            0f to "DEFAULT",
            if (canPrev) { widgetWidth.toFloat() to "PREV" } else null,
            if (canNext) { -widgetWidth.toFloat() to "NEXT" } else null,
        ).toMap()

        anchoredDraggableState.updateAnchors(
            anchors,
            {value ->
                if (value == widgetWidth.toFloat()) {
                    onPrev()
                    anchoredDraggableState.update(0f)
                } else if (value == -widgetWidth.toFloat()) {
                    onNext()
                    anchoredDraggableState.update(0f)
                }
            }
        )
    }

    LaunchedEffect(canPrev, canNext) {
        updateAnchored()
    }

    Box(
        modifier = Modifier
            .onSizeChanged { size ->
                if (widgetWidth != size.width) {
                    widgetWidth = size.width;
                    updateAnchored()
                }
            }
            .customAnchoredDraggable(
                state = anchoredDraggableState,
                orientation = Orientation.Horizontal,
                onDragStarted = {
                    dragStartX = anchoredDraggableState.value
                },
                onLimitDragEnded = {nextValue ->
                    val dis = (nextValue - dragStartX).absoluteValue.coerceIn(0f, widgetWidth.toFloat());
                    val sign = (nextValue - dragStartX).sign;
                    val next = dragStartX + dis * sign
                    next
                }
            )
            .fillMaxSize()
    ) {
        if (widgetWidth > 0) {
            if (canPrev) {
                Box(
                    modifier = Modifier
                        .offset(x = -widgetWidthDp + deltaDp),
                    contentAlignment = Alignment.Center,
                ) {
                    CoverImage(url = prevCover)
                }
            }
            if (canNext) {
                Box(
                    modifier = Modifier
                        .offset(x = widgetWidthDp + deltaDp),
                    contentAlignment = Alignment.Center,
                ) {
                    CoverImage(url = nextCover)
                }
            }
        }
        Box(
            modifier = Modifier
                .offset(x = deltaDp),
            contentAlignment = Alignment.Center,
        ) {
            CoverImage(url = cover)
        }
    }
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

    Box(
        modifier = Modifier
            .background(Color.White)
            .fillMaxSize()
    ) {
        Column {
            MusicPlayerHeader(
                onRemoveClick = {
                    Bridge.dispatchClick(MusicDetailWidget.REMOVE)
                },
            )
            Column(
                modifier = Modifier
                    .weight(1.0F)
            ) {
                MusicPlayerBody(
                    onPrev = {
                        Bridge.dispatchClick(MusicControlWidget.PLAY_PREVIOUS)
                    },
                    onNext = {
                        Bridge.dispatchClick(MusicControlWidget.PLAY_NEXT)
                    },
                    cover = state.cover,
                    prevCover = state.previousCover,
                    nextCover = state.nextCover,
                    canPrev = state.canPlayPrevious,
                    canNext = state.canPlayNext
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
                    _currentDurationMS = state.currentDurationMs,
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
            _currentDurationMS = currentMS,
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
private fun MusicPlayerBodyPreview() {
    var canPrev by remember { mutableStateOf(true) }
    var canNext by remember { mutableStateOf(true) }

    Column(
        modifier = Modifier
            .fillMaxSize()
    ) {
        Row {
            Column {
                Text(text = "canPrev")
                Switch(
                    checked = canPrev,
                    onCheckedChange = {value -> canPrev = value}
                )
            }
            Column {
                Text(text = "canNext")
                Switch(
                    checked = canNext,
                    onCheckedChange = {value -> canNext = value}
                )
            }
        }
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .background(Color.Blue)
        )
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier.weight(1f)
        ) {
            MusicPlayerBody(
                onPrev = {
                },
                onNext = {
                },
                cover = "",
                prevCover = "",
                nextCover = "",
                canPrev = canPrev,
                canNext = canNext
            )
        }
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .background(Color.Blue))
    }
}
