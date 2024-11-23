package com.kutedev.easemusicplayer.widgets

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import kotlinx.coroutines.delay
import kotlin.math.PI
import kotlin.math.sin

@Composable
fun LoadingPage(
    evm: EaseViewModel,
    block: @Composable () -> Unit
) {
    val state by evm.mainState.collectAsState()
    val isLoading = !state.vsLoaded

    LoadingPageImpl(
        isLoading,
        block,
    )
}

@Composable
private fun LoadingPageImpl(
    loading: Boolean,
    block: @Composable () -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(color = MaterialTheme.colorScheme.surface),
        contentAlignment = Alignment.Center
    ) {
        if (loading) {
            CustomLoadingAnimation(
                modifier = Modifier
                    .size(100.dp)
            )
        } else {
            block()
        }
    }
}



@Composable
private fun CustomLoadingAnimation(
    modifier: Modifier = Modifier
) {
    val infiniteTransition = rememberInfiniteTransition(label = "")
    val t by infiniteTransition.animateFloat(
        initialValue = 0f,
        targetValue = 2 * PI.toFloat(),
        animationSpec = infiniteRepeatable(
            animation = tween(
                durationMillis = 1200,
                easing = LinearEasing
            ),
            repeatMode = RepeatMode.Restart
        ),
        label = ""
    )
    val SIZE = 16.dp
    val GAP = 36.dp

    val scale = sin(t)
    val prevScale = sin(t + 1.5f)
    val nextScale = sin(t - 1.5f)

    Box(
        modifier = Modifier
            .offset(x = -GAP)
            .clip(RoundedCornerShape(999.dp))
            .background(MaterialTheme.colorScheme.primary)
            .size(SIZE * prevScale)
    )
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(999.dp))
            .background(MaterialTheme.colorScheme.primary)
            .size(SIZE * scale)
    )
    Box(
        modifier = Modifier
            .offset(x = GAP)
            .clip(RoundedCornerShape(999.dp))
            .background(MaterialTheme.colorScheme.primary)
            .size(SIZE * nextScale)
    )
}


@Preview
@Composable
private fun LoadingPreview() {
    var loading by remember { mutableStateOf(true) }

    Column {
        Switch(
            checked = loading,
            onCheckedChange = { value ->
                loading = value
            }
        )
        Box(
            modifier = Modifier
                .size(400.dp)
        ) {
            LoadingPageImpl(loading) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Text("123")
                }
            }
        }
    }
}