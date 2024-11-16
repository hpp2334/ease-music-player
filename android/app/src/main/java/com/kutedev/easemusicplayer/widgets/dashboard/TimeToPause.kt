package com.kutedev.easemusicplayer.widgets.dashboard

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.core.graphics.ColorUtils
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.core.UIBridgeController
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.ease_client.TimeToPauseAction
import uniffi.ease_client.TimeToPauseWidget
import uniffi.ease_client.ViewAction
import kotlin.math.absoluteValue
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.min

@Composable
private fun Block(
    stringId: Int,
    l: Int,
    r: Int,
    current: Int,
    onChange: (value: Int) -> Unit
) {
    val BOX_WIDTH = 56.dp
    val BOX_HEIGHT = 150.dp
    val STRIDE = 50;
    var dragOffsetInDp by remember { mutableStateOf(0f) }
    val density = LocalDensity.current
    val rng = r - l + 1;

    fun next(x: Int): Int {
        return ((current + x - l) % rng + rng) % rng + l;
    }

    val consumeDragOffsetInDp = {
        val gapFloor = floor(dragOffsetInDp / STRIDE).toInt()
        val gapCeil = ceil(dragOffsetInDp / STRIDE).toInt()

        var gap = 0
        var isFloor = false
        if (dragOffsetInDp - gapFloor * STRIDE < gapCeil * STRIDE - dragOffsetInDp) {
            gap = gapFloor
            isFloor = true
        } else {
            gap = gapCeil
            isFloor = false
        }

        if (gap != 0) {
            val next = next(-gap);

            if (isFloor) {
                dragOffsetInDp -= gapFloor * STRIDE;
            } else {
                dragOffsetInDp -= gapCeil * STRIDE;
            }
            onChange(next)
        }
    }

    val draggableState = rememberDraggableState { deltaPx ->
        dragOffsetInDp += with(density) { deltaPx.toDp().value }

        consumeDragOffsetInDp()
    }

    // Coroutine scope to manage the animation
    val animationScope = rememberCoroutineScope()
    var animationJob by remember { mutableStateOf<Job?>(null) }

    val startAnimateDragOffsetToZero = {
        // Cancel any ongoing animation
        animationJob?.cancel()

        consumeDragOffsetInDp()

        // Start a new animation
        animationJob = animationScope.launch {
            animate(dragOffsetInDp, 0f, animationSpec = tween(durationMillis = 300, easing = FastOutSlowInEasing)) { value, _ ->
                dragOffsetInDp = value
            }
        }
    }
    val abortAnimateDragOffset = {
        // Cancel any ongoing animation when dragging starts
        animationJob?.cancel()
    }

    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(stringId),
            fontSize = 9.sp,
        )
        Box(
            modifier = Modifier
                .width(BOX_WIDTH)
                .height(BOX_HEIGHT)
                .clipToBounds()
                .draggable(
                    state = draggableState,
                    orientation = Orientation.Vertical,
                    onDragStarted = {
                        abortAnimateDragOffset()
                    },
                    onDragStopped = {
                        startAnimateDragOffsetToZero()
                    }
                ),
            contentAlignment = Alignment.Center,
        ) {
            for (i in -2..2) {
                val dis = (i * STRIDE) + dragOffsetInDp
                val offsetY = dis.dp + 10.dp
                val color = MaterialTheme.colorScheme.onSurface.copy(
                    alpha = 1 - 0.5f * min(dis.absoluteValue / STRIDE, 1f)
                )
                val fontSizeValue = 36 - 6 * min(dis.absoluteValue / STRIDE, 1f)

                Text(
                    modifier = Modifier
                        .padding(vertical = 16.dp)
                        .offset(0.dp, offsetY),
                    fontSize = fontSizeValue.sp,
                    color = color,
                    text = next(i).toString().padStart(2, '0'),
                )
            }
        }
    }
}

@Composable
private fun TimeToPauseModalCore(
    isOpen: Boolean,
    initHours: Int,
    initMinutes: Int,
    deleteEnabled: Boolean,
    onCancel: () -> Unit,
    onConfirm: (Int, Int) -> Unit,
    onDelete: () -> Unit,
) {
    var hours by remember { mutableIntStateOf(0) }
    var minutes by remember { mutableIntStateOf(0) }

    LaunchedEffect(isOpen) {
        hours = initHours
        minutes = initMinutes
    }

    if (!isOpen) {
        return
    }

    Dialog(
        onDismissRequest = onCancel,
    ) {
        Column(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(24.dp, 24.dp),
        ) {
            Row(
                modifier = Modifier
                    .align(Alignment.CenterHorizontally)
            ) {
                Block(
                    stringId = R.string.time_to_pause_hour,
                    l = 0,
                    r = 99,
                    current = hours,
                    onChange = { value -> hours = value }
                )
                Box(
                    modifier = Modifier
                        .width(20.dp)
                )
                Block(
                    stringId = R.string.time_to_pause_minute,
                    l = 0,
                    r = 59,
                    current = minutes,
                    onChange = { value -> minutes = value }
                )
            }
            Box(
                modifier = Modifier
                    .height(16.dp)
            )
            Row(
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier
                    .fillMaxWidth()
            ) {
                Row {
                    EaseTextButton(
                        text = stringResource(id = R.string.time_to_pause_delete),
                        type = EaseTextButtonType.Error,
                        size = EaseTextButtonSize.Medium,
                        disabled = !deleteEnabled,
                        onClick = {
                            onDelete()
                        }
                    )
                }
                Row {
                    EaseTextButton(
                        text = stringResource(id = R.string.playlists_dialog_button_cancel),
                        type = EaseTextButtonType.Primary,
                        size = EaseTextButtonSize.Medium,
                        onClick = {
                            onCancel()
                        }
                    )
                    EaseTextButton(
                        text = stringResource(id = R.string.playlists_dialog_button_ok),
                        type = EaseTextButtonType.Primary,
                        size = EaseTextButtonSize.Medium,
                        disabled = minutes == 0 && hours == 0,
                        onClick = {
                            onConfirm(hours, minutes)
                        }
                    )
                }
            }
        }
    }
}

@Composable
fun TimeToPauseModal(
    evm: EaseViewModel,
) {
    val bridge = UIBridgeController.current
    val state by evm.timeToPauseState.collectAsState()
    val onClose = {
        bridge.dispatchAction(ViewAction.TimeToPause(TimeToPauseAction.CloseModal));
    }

    TimeToPauseModalCore(
        isOpen = state.modalOpen,
        initHours = state.leftHour.toInt(),
        initMinutes = state.leftMinute.toInt(),
        deleteEnabled = state.enabled,
        onCancel = onClose,
        onConfirm = { hour, minute ->
            bridge.dispatchAction(ViewAction.TimeToPause(TimeToPauseAction.Finish(hour.toUByte(), minute.toUByte(), 0u)))
            onClose()
        },
        onDelete = {
            bridge.dispatchClick(TimeToPauseWidget.DELETE)
            onClose()
        }
    )
}

@Composable
@Preview
private fun TimeToPauseModalPreview() {
    var isOpen by remember { mutableStateOf(true) }

    val close = {
        isOpen = false;
    };

    EaseTextButton(
        text = "Open",
        type = EaseTextButtonType.Primary,
        size = EaseTextButtonSize.Medium,
        onClick = {
            isOpen = true;
        },
        disabled = false,
    )
    TimeToPauseModalCore(
        isOpen = isOpen,
        initHours = 0,
        initMinutes = 0,
        deleteEnabled = true,
        onCancel = {
            close()
        },
        onConfirm = { v1, v2 ->
            close()
        },
        onDelete = {
            close()
        }
    )
}