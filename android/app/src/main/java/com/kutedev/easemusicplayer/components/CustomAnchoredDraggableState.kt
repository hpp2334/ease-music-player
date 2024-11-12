package com.kutedev.easemusicplayer.components

import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.DraggableState
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

private fun findNearestKey(map: Map<Float, Any>, x: Float): Float {
    var nearestKey: Float = x
    var smallestDifference = Float.MAX_VALUE

    for (key in map.keys) {
        val difference = kotlin.math.abs(key - x)
        if (difference < smallestDifference) {
            smallestDifference = difference
            nearestKey = key
        }
    }

    return nearestKey
}

class CustomAnchoredDraggableState(
    initialValue: Float,
    private val coroutineScope: CoroutineScope,
    private var anchors: Map<Float, Any>,
    private val animationSpec: AnimationSpec<Float>,
    private var onAnimatedChange: (value: Float) -> Unit,
)  {
    private var animationJob: Job? = null
    var value by mutableStateOf(initialValue)
        private set
    val draggableState = DraggableState { delta ->
        value += delta
    }

    private fun animateToImpl(target: Float) {
        animationJob?.cancel()
        animationJob = coroutineScope.launch {
            val nearestAnchor = findNearestKey(anchors, target)
            animate(value, nearestAnchor, animationSpec = animationSpec) { nextValue, _ ->
                value = nextValue
                onAnimatedChange(value)
            }
        }
    }

    fun update(newValue: Float) {
        animationJob?.cancel()
        value = newValue
    }

    fun animateTo(newValue: Float) {
        animateToImpl(newValue)
    }

    fun updateAnchors(anchors: Map<Float, Any>, onChange: (value: Float) -> Unit) {
        animationJob?.cancel()
        this.anchors = anchors
        this.onAnimatedChange = onChange
        animateToImpl(value)
    }

    fun onDragStopped(value: Float) {
        animateToImpl(value)
    }
}

@Composable
fun rememberCustomAnchoredDraggableState(
    initialValue: Float,
    anchors: Map<Float, Any>,
    animationSpec: AnimationSpec<Float>,
    onChange: (value: Float) -> Unit = {}
): CustomAnchoredDraggableState {
    val coroutineScope = rememberCoroutineScope()
    val state = remember {
        CustomAnchoredDraggableState(
            initialValue,
            coroutineScope,
            anchors,
            animationSpec,
            onChange,
        )
    }
    return state
}

fun Modifier.customAnchoredDraggable(
    state: CustomAnchoredDraggableState,
    orientation: Orientation,
    onDragStarted: () -> Unit = {},
    onLimitDragEnded: (nextValue: Float) -> Float = { value -> value }
): Modifier {
    return this.then(
        Modifier
            .draggable(
                state = state.draggableState,
                orientation = orientation,
                onDragStarted = { onDragStarted() },
                onDragStopped = { velocity ->
                    var nextValue = state.value + velocity * 0.5f
                    nextValue = onLimitDragEnded(nextValue)
                    state.onDragStopped(nextValue)
                }
            )
    )
}


@Preview
@Composable
private fun CustomAnchoredDraggableStatePreview() {
    val anchors = mapOf(
        -50f to "Negative",
        0f to "Start",
        100f to "Middle",
        200f to "End"
    )
    val state = rememberCustomAnchoredDraggableState(initialValue = 0f, anchors = anchors, animationSpec = tween(durationMillis = 300, easing = FastOutSlowInEasing))

    Column {
        Box(
            modifier = Modifier
                .width(200.dp)
                .height(50.dp),
            contentAlignment = Alignment.Center,
        ) {
            Box(
                modifier = Modifier
                    .customAnchoredDraggable(state = state, orientation = Orientation.Horizontal)
                    .background(Color.Red)
                    .size(50.dp)
            )
        }
        Text(
            text = state.value.toString()
        )
    }
}
