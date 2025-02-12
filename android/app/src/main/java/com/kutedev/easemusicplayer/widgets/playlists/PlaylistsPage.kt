package com.kutedev.easemusicplayer.widgets.playlists

import EaseImage
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import uniffi.ease_client.VPlaylistAbstractItem
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.UIBridgeController
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBarSpacer
import uniffi.ease_client.PlaylistListWidget

@Composable
fun PlaylistsSubpage(
    evm: EaseViewModel,
) {
    val bridge = UIBridgeController.current
    val state = evm.playlistListState.collectAsState().value

    if (state.playlistList.isEmpty()) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier.fillMaxSize()
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier
                    .clickable {
                        bridge.dispatchClick(PlaylistListWidget.Add)
                    }
                    .clip(RoundedCornerShape(16.dp))
                    .padding(24.dp, 24.dp),
            ) {
                Image(painter = painterResource(id = R.drawable.empty_playlists), contentDescription = null)
                Box(modifier = Modifier.height(20.dp))
                Text(
                    text = stringResource(id = R.string.playlist_empty),
                )
            }
        }
    } else {
        Column(
            modifier = Modifier
                .fillMaxSize()
        ) {
            Row(
                modifier = Modifier
                    .padding(24.dp, 8.dp)
                    .fillMaxWidth(),
                horizontalArrangement = Arrangement.End
            ) {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_plus),
                    onClick = {
                        bridge.dispatchClick(PlaylistListWidget.Add)
                    }
                )
            }
            GridPlaylists(playlists = state.playlistList)
        }
    }
}

@Composable
private fun GridPlaylists(playlists: List<VPlaylistAbstractItem>) {
    LazyVerticalGrid(
        modifier = Modifier.fillMaxSize(),
        columns = GridCells.FixedSize(172.dp),
        horizontalArrangement = Arrangement.Center,
    ) {
        items(playlists) { playlist ->
            PlaylistItem(playlist = playlist)
        }
    }
}

@Composable
private fun PlaylistItem(playlist: VPlaylistAbstractItem) {
    val bridge = UIBridgeController.current
    Box(Modifier
        .clickable(
            onClick = {
                bridge.dispatchClick(PlaylistListWidget.Item(playlist.id));
            },
        )
    ) {
        Column(
            modifier = Modifier.padding(12.dp, 8.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.Start
        ) {
            Box(
                modifier = Modifier.clip(RoundedCornerShape(20.dp)).background(MaterialTheme.colorScheme.onSurfaceVariant).size(136.dp)
            ) {
                if (playlist.cover == null) {
                    Image(
                        modifier = Modifier.fillMaxSize(),
                        painter = painterResource(id = R.drawable.cover_default_image),
                        contentDescription = null,
                        contentScale = ContentScale.FillWidth
                    )
                } else {
                    EaseImage(
                        modifier = Modifier.fillMaxSize(),
                        dataSourceKey = playlist.cover!!,
                        contentScale = ContentScale.FillWidth
                    )
                }
            }
            Text(
                text = playlist.title,
                fontSize = 14.sp,
                modifier = Modifier.padding(top = 8.dp),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                text = buildAnnotatedString {
                    append("${playlist.count} ${stringResource(id = R.string.music_count_unit)}")
                    append("  ·  ")
                    append(playlist.duration)
                },
                fontSize = 12.sp,
                fontWeight = FontWeight.Light,
                maxLines = 1,
            )
        }
    }
}