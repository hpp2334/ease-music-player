package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.MusicCover
import com.kutedev.easemusicplayer.viewmodels.PlayerVM
import com.kutedev.easemusicplayer.widgets.LocalNavController
import com.kutedev.easemusicplayer.widgets.RouteMusicPlayer
import uniffi.ease_client_schema.DataSourceKey

@Composable
private fun MiniPlayerCore(
    isPlaying: Boolean,
    title: String,
    cover: DataSourceKey?,
    currentDurationMS: ULong,
    totalDuration: String,
    totalDurationMS: ULong,
    loading: Boolean,
    canNext: Boolean,
    onClick: () -> Unit,
    onPlay: () -> Unit,
    onPause: () -> Unit,
    onStop: () -> Unit,
    onNext: () -> Unit,
) {
    Row(
        modifier = Modifier.clickable { onClick() }.fillMaxWidth().padding(30.dp).height(64.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        MusicCover(
            modifier = Modifier
                .clip(RoundedCornerShape(10.dp))
                .size(60.dp),
            coverDataSourceKey = cover,
        )
        Box(modifier = Modifier.width(16.dp))
        Column(
            modifier = Modifier.fillMaxHeight(),
            horizontalAlignment = Alignment.End,
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    modifier = Modifier.weight(1f),
                    text = title,
                    style = TextStyle(fontSize = 16.sp),
                    overflow = TextOverflow.Ellipsis,
                    maxLines = 1
                )
                Box(modifier = Modifier.width(16.dp))
                Row(
                    modifier = Modifier.wrapContentWidth()
                ) {
                    if (!isPlaying) {
                        EaseIconButton(
                            sizeType = EaseIconButtonSize.Medium,
                            buttonType = EaseIconButtonType.Default,
                            disabled = loading,
                            painter = painterResource(R.drawable.icon_play),
                            onClick = onPlay,
                        )
                    } else {
                        EaseIconButton(
                            sizeType = EaseIconButtonSize.Medium,
                            buttonType = EaseIconButtonType.Default,
                            painter = painterResource(R.drawable.icon_pause),
                            onClick = onPause,
                        )
                    }
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Default,
                        painter = painterResource(R.drawable.icon_play_next),
                        disabled = !canNext,
                        onClick = onNext,
                    )
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Default,
                        painter = painterResource(R.drawable.icon_stop),
                        onClick = onStop,
                    )
                }
            }
            Box(modifier = Modifier.height(4.dp))
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(999.dp))
                    .fillMaxWidth()
            ) {
                LinearProgressIndicator(
                    modifier = Modifier.fillMaxWidth(),
                    progress = {
                        if (totalDurationMS == 0uL) {
                            0f
                        } else {
                            currentDurationMS.toFloat() / totalDurationMS.toFloat()
                        }
                    },
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
            Text(
                text = totalDuration,
                fontSize = 9.sp,
            )
        }
    }
}

@Composable
fun MiniPlayer(
    playerVM: PlayerVM = viewModel()
) {
    val state = playerVM.musicState.collectAsState().value
    val navController = LocalNavController.current

    MiniPlayerCore(
        isPlaying = state.playing,
        title = state.title,
        cover = state.cover,
        currentDurationMS = state.currentDurationMs,
        totalDuration = state.totalDuration,
        totalDurationMS = state.totalDurationMs,
        canNext = state.canPlayNext,
        loading = state.loading,
        onClick = { navController.navigate(RouteMusicPlayer) },
        onPlay = { playerVM.resume() },
        onPause = { playerVM.pause() },
        onStop = { playerVM.stop() },
        onNext = { playerVM.playNext() },
    )
}

@Preview
@Composable
private fun MiniPlayerPreview() {
    MiniPlayerCore(
        isPlaying = true,
        title = "Very very very very very long music title",
        cover = null,
        currentDurationMS = 10uL,
        totalDuration = "00:06",
        totalDurationMS = 60uL,
        canNext = false,
        loading = false,
        onClick = {},
        onPlay = {},
        onPause = {},
        onStop = {},
        onNext = {}
    )
}