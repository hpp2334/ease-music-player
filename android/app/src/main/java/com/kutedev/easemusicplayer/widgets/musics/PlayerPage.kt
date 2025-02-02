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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
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
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
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
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.MusicCover
import com.kutedev.easemusicplayer.components.customAnchoredDraggable
import com.kutedev.easemusicplayer.components.dropShadow
import com.kutedev.easemusicplayer.components.rememberCustomAnchoredDraggableState
import com.kutedev.easemusicplayer.core.UIBridgeController
import com.kutedev.easemusicplayer.utils.nextTickOnMain
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import uniffi.ease_client.MusicControlAction
import uniffi.ease_client.MusicControlWidget
import uniffi.ease_client.MusicDetailWidget
import uniffi.ease_client.MusicLyricWidget
import uniffi.ease_client.VLyricLine
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.DataSourceKey
import uniffi.ease_client_shared.LyricLoadState
import uniffi.ease_client_shared.PlayMode
import kotlin.math.absoluteValue
import kotlin.math.sign

@Composable
private fun MusicPlayerHeader(
    hasLyric: Boolean,
) {
    val bridge = UIBridgeController.current
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
                bridge.popRoute()
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
                        if (hasLyric) {
                            EaseContextMenuItem(
                                stringId = R.string.music_lyric_remove,
                                onClick = {
                                    bridge.dispatchClick(MusicLyricWidget.REMOVE)
                                }
                            )
                        } else {
                            EaseContextMenuItem(
                                stringId = R.string.music_lyric_add,
                                onClick = {
                                    bridge.dispatchClick(MusicLyricWidget.ADD)
                                }
                            )
                        },
                        EaseContextMenuItem(
                            stringId = R.string.music_player_context_menu_remove,
                            isError = true,
                            onClick = {
                                bridge.dispatchClick(MusicDetailWidget.REMOVE)
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
    bufferDurationMS: ULong,
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
    val bufferRate = if (totalDurationMS == 0UL) { 0f } else { (bufferDurationMS.toDouble() / totalDurationMS.toDouble()).toFloat() };
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
                    if (sliderWidth != size.width) {
                        sliderWidth = size.width;
                    }
                }
                .pointerInput(totalDurationMS, sliderWidth) {
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
            Box(modifier = Modifier
                .fillMaxWidth()
                .height(sliderHeight)
                .offset(0.dp, (sliderContainerHeight - sliderHeight) / 2)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
            ) {
                Box(modifier = Modifier
                    .fillMaxWidth(bufferRate)
                    .fillMaxHeight()
                    .background(MaterialTheme.colorScheme.secondary)
                )
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
private fun CoverImage(dataSourceKey: DataSourceKey?) {
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
            coverDataSourceKey = dataSourceKey,
        )
    }
}

@Composable
private fun MusicLyric(
    lyrics: List<VLyricLine>,
    lyricIndex: Int,
    lyricLoadedState: LyricLoadState,
    onClickAdd: () -> Unit,
    widgetHeight: Int,
) {
    val density = LocalDensity.current
    val widgetHeightDp = with(density) {
        widgetHeight.toDp()
    }
    val listState = rememberLazyListState()

    LaunchedEffect(lyricIndex, widgetHeight, lyricLoadedState) {
        if (lyricLoadedState == LyricLoadState.LOADED) {
            listState.animateScrollToItem(lyricIndex + 1, -(widgetHeight / 2))
        }
    }

    if (widgetHeight == 0) {
        return
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(32.dp),
        contentAlignment = Alignment.Center
    ) {
        if (lyricLoadedState == LyricLoadState.MISSING || lyricLoadedState == LyricLoadState.FAILED) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Icon(
                    modifier = Modifier.size(64.dp),
                    painter = painterResource(R.drawable.icon_lyrics),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.surfaceVariant
                )
                Box(modifier = Modifier.height(4.dp))
                if (lyricLoadedState == LyricLoadState.MISSING) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = stringResource(R.string.music_lyric_no_desc),
                            fontSize = 14.sp,
                        )
                        EaseTextButton(
                            text = stringResource(R.string.music_lyric_try_add_desc),
                            type = EaseTextButtonType.Primary,
                            size = EaseTextButtonSize.Medium,
                            onClick = {
                                onClickAdd()
                            }
                        )
                    }
                } else {
                    Text(
                        text = stringResource(R.string.music_lyric_fail),
                        fontSize = 14.sp,
                    )
                }
            }
            return
        }

        if (lyricLoadedState == LyricLoadState.LOADING) {
            return
        }

        Column {
            LazyColumn(
                state = listState,
                modifier = Modifier
                    .fillMaxWidth(),
                userScrollEnabled = false,
            ) {
                item {
                    Box(modifier = Modifier.height(widgetHeightDp / 2))
                }
                itemsIndexed(lyrics) { index, lyric ->
                    val isCurrent = index == lyricIndex
                    val textColor = if (isCurrent) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant

                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 8.dp)
                    ) {
                        Text(
                            text = lyric.text,
                            color = textColor,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.align(Alignment.CenterStart)
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun MusicPlayerBody(
    onPrev: () -> Unit,
    onNext: () -> Unit,
    cover: DataSourceKey?,
    prevCover: DataSourceKey?,
    nextCover: DataSourceKey?,
    canPrev: Boolean,
    canNext: Boolean,
    lyricIndex: Int,
    lyrics: List<VLyricLine>,
    lyricLoadedState: LyricLoadState,
    onClickAddLyric: () -> Unit,
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
    var widgetHeight by remember { mutableIntStateOf(0) }

    var dragStartX by remember { mutableFloatStateOf(0f) }
    var showLyric by remember { mutableStateOf(false) }

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
                    nextTickOnMain {
                        onPrev()
                        anchoredDraggableState.update(0f)
                        showLyric = false
                    }
                } else if (value == -widgetWidth.toFloat()) {
                    nextTickOnMain {
                        onNext()
                        anchoredDraggableState.update(0f)
                        showLyric = false
                    }
                }
            }
        )
    }

    LaunchedEffect(canPrev, canNext) {
        updateAnchored()
    }

    Box(
        modifier = Modifier
            .pointerInput(Unit) {
                detectTapGestures {
                    showLyric = !showLyric
                }
            }
            .onSizeChanged { size ->
                if (widgetWidth != size.width) {
                    widgetWidth = size.width;
                    updateAnchored()
                }
                if (widgetHeight != size.height) {
                    widgetHeight = size.height
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
                    CoverImage(dataSourceKey = prevCover)
                }
            }
            if (canNext) {
                Box(
                    modifier = Modifier
                        .offset(x = widgetWidthDp + deltaDp),
                    contentAlignment = Alignment.Center,
                ) {
                    CoverImage(dataSourceKey = nextCover)
                }
            }
        }
        Box(
            modifier = Modifier
                .offset(x = deltaDp),
            contentAlignment = Alignment.Center,
        ) {
            if (!showLyric) {
                CoverImage(dataSourceKey = cover)
            } else {
                MusicLyric(
                    lyricIndex = lyricIndex,
                    lyrics = lyrics,
                    lyricLoadedState = lyricLoadedState,
                    onClickAdd = onClickAddLyric,
                    widgetHeight = widgetHeight,
                )
            }
        }
    }
}

@Composable
private fun MusicPanel(
    evm: EaseViewModel,
) {
    val bridge = UIBridgeController.current
    val state by evm.currentMusicState.collectAsState()
    val timeToPauseState by evm.timeToPauseState.collectAsState()
    val isTimeToPauseOpen = timeToPauseState.enabled
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
            buttonType = if (isTimeToPauseOpen) { EaseIconButtonType.Primary } else { EaseIconButtonType.Default },
            overrideColors = EaseIconButtonColors(
                iconTint = if (isTimeToPauseOpen) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onSurface
                },
                buttonBg = Color.Transparent,
            ),
            painter = painterResource(id = R.drawable.icon_timelapse),
            onClick = {
                bridge.dispatchClick(MusicControlWidget.TIME_TO_PAUSE)
            }
        )
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = R.drawable.icon_play_previous),
            disabled = !state.canPlayPrevious,
            onClick = {
                bridge.dispatchClick(MusicControlWidget.PLAY_PREVIOUS)
            }
        )
        if (!state.playing) {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Large,
                buttonType = EaseIconButtonType.Primary,
                painter = painterResource(id = R.drawable.icon_play),
                disabled = state.loading,
                overrideColors = if (state.loading) {
                    EaseIconButtonColors(
                        buttonDisabledBg = MaterialTheme.colorScheme.secondary,
                    )
                } else { null },
                onClick = {
                    bridge.dispatchClick(MusicControlWidget.PLAY)
                }
            )
        }
        if (state.playing) {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Large,
                buttonType = EaseIconButtonType.Primary,
                painter = painterResource(id = R.drawable.icon_pause),
                onClick = {
                    bridge.dispatchClick(MusicControlWidget.PAUSE)
                }
            )
        }
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = R.drawable.icon_play_next),
            disabled = !state.canPlayNext,
            onClick = {
                bridge.dispatchClick(MusicControlWidget.PLAY_NEXT)
            }
        )
        EaseIconButton(
            sizeType = EaseIconButtonSize.Medium,
            buttonType = EaseIconButtonType.Default,
            painter = painterResource(id = modeDrawable),
            onClick = {
                bridge.dispatchClick(MusicControlWidget.PLAYMODE)
            }
        )
    }
}

@Composable
fun MusicPlayerPage(
    evm: EaseViewModel,
) {
    val bridge = UIBridgeController.current
    val currentMusicState by evm.currentMusicState.collectAsState()
    val currentLyricState by evm.currentMusicLyricState.collectAsState()

    val hasLyric = currentLyricState.loadState != LyricLoadState.MISSING

    Box(
        modifier = Modifier
            .clipToBounds()
            .background(Color.White)
            .fillMaxSize()
    ) {
        Column {
            MusicPlayerHeader(
                hasLyric = hasLyric,
            )
            Column(
                modifier = Modifier
                    .weight(1.0F)
            ) {
                MusicPlayerBody(
                    onPrev = {
                        bridge.dispatchClick(MusicControlWidget.PLAY_PREVIOUS)
                    },
                    onNext = {
                        bridge.dispatchClick(MusicControlWidget.PLAY_NEXT)
                    },
                    cover = currentMusicState.cover,
                    prevCover = currentMusicState.previousCover,
                    nextCover = currentMusicState.nextCover,
                    canPrev = currentMusicState.canPlayPrevious,
                    canNext = currentMusicState.canPlayNext,
                    lyricIndex = currentMusicState.lyricIndex,
                    lyricLoadedState = currentLyricState.loadState,
                    lyrics = currentLyricState.lyricLines,
                    onClickAddLyric = {
                        bridge.dispatchClick(MusicLyricWidget.ADD)
                    }
                )
            }
            Column(
                modifier = Modifier.padding(36.dp, 10.dp)
            ) {
                Text(
                    text = currentMusicState.title,
                    maxLines = 3,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontSize = 20.sp,
                    modifier = Modifier.padding(0.dp, 10.dp)
                )
                MusicSlider(
                    currentDuration = currentMusicState.currentDuration,
                    _currentDurationMS = currentMusicState.currentDurationMs,
                    bufferDurationMS = currentMusicState.bufferDurationMs,
                    totalDuration = currentMusicState.totalDuration,
                    totalDurationMS = currentMusicState.totalDurationMs,
                    onChangeMusicPosition = { nextMS ->
                        bridge.dispatchAction(ViewAction.MusicControl(MusicControlAction.Seek(nextMS)))
                    }
                )
            }
            Row(
                modifier = Modifier
                    .align(Alignment.CenterHorizontally)
                    .padding(0.dp, 48.dp)
            ) {
                MusicPanel(
                    evm = evm,
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
    val bufferMS by remember {
        mutableStateOf(70uL * 1000uL)
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
            bufferDurationMS = bufferMS,
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
                cover = null,
                prevCover = null,
                nextCover = null,
                canPrev = canPrev,
                canNext = canNext,
                lyricIndex = 0,
                lyrics = listOf(),
                lyricLoadedState = LyricLoadState.LOADING,
                onClickAddLyric = {},
            )
        }
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .background(Color.Blue))
    }
}


@Preview()
@Composable
private fun MusicLyricPreview() {
    var lyricLoadedState by remember { mutableStateOf(LyricLoadState.LOADED) }
    var lyricIndex by remember { mutableIntStateOf(0) }
    val lyricLines = remember {
        listOf(
            VLyricLine(1000u, "> Task :app:preBuild UP-TO-DATE"),
            VLyricLine(3000u, "> Task :app:preDebugBuild UP-TO-DATE"),
            VLyricLine(4000u, "> Task :app:mergeDebugNativeDebugMetadata NO-SOURCE"),
            VLyricLine(4500u, "> Task :app:checkDebugAarMetadata UP-TO-DATE"),
            VLyricLine(5000u, "> Task :app:generateDebugResValues UP-TO-DATE"),
            VLyricLine(5500u, "For more on this, please refer to https://docs.gradle.org/8.9/userguide/command_line_interface.html#sec:command_line_warnings in the Gradle documentation."),
            VLyricLine(6000u, "> Task :app:generateDebugResValues UP-TO-DATE"),
            VLyricLine(7000u, "You can use '--warning-mode all' to show the individual deprecation warnings and determine if they come from your own scripts or plugins."),
            VLyricLine(8000u, "> Task :app:createDebugApkListingFileRedirect UP-TO-DATE"),
            VLyricLine(9000u, "> Task :app:assembleDebug"),
        )
    }
    var widgetHeight by remember { mutableIntStateOf(0) }

    Column(
        modifier = Modifier
            .width(400.dp)
            .height(600.dp)
    ) {
        Row {
            Button(
                onClick = {
                    if (lyricIndex > 0) {
                        lyricIndex -= 1
                    }
                }
            ) {
                Text(text = "-")
            }
            Button(
                onClick = {
                    if (lyricIndex < lyricLines.size - 1) {
                        lyricIndex += 1
                    }
                }
            ) {
                Text(text = "+")
            }
        }
        Row {
            Button(onClick = { lyricLoadedState = LyricLoadState.MISSING }) { Text("MISSING") }
            Button(onClick = { lyricLoadedState = LyricLoadState.LOADING }) { Text("LOADING") }
            Button(onClick = { lyricLoadedState = LyricLoadState.LOADED }) { Text("LOADED") }
            Button(onClick = { lyricLoadedState = LyricLoadState.FAILED }) { Text("FAILED") }
        }
        Box(modifier = Modifier
            .onSizeChanged { size ->
                if (size.height != widgetHeight) {
                    widgetHeight = size.height
                }
            }
            .fillMaxSize()
        ) {
            MusicLyric(
                lyricIndex = lyricIndex,
                lyrics = lyricLines,
                lyricLoadedState = lyricLoadedState,
                widgetHeight = widgetHeight,
                onClickAdd = {}
            )
        }
    }
}
