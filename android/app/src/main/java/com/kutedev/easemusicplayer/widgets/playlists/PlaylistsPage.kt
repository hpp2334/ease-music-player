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
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistsVM
import com.kutedev.easemusicplayer.viewmodels.durationStr
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RoutePlaylist
import com.kutedev.easemusicplayer.viewmodels.PlaylistsMode
import sh.calvin.reorderable.ReorderableCollectionItemScope
import sh.calvin.reorderable.ReorderableItem
import sh.calvin.reorderable.ScrollMoveMode
import sh.calvin.reorderable.rememberReorderableLazyGridState
import uniffi.ease_client_backend.PlaylistAbstract

@Composable
fun PlaylistsSubpage(
    playlistsVM: PlaylistsVM = hiltViewModel(),
    editPlaylistVM: CreatePlaylistVM = hiltViewModel()
) {
    val playlists by playlistsVM.playlists.collectAsState()
    val playlistsMode by playlistsVM.mode.collectAsState()

    if (playlists.isEmpty()) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier.fillMaxSize()
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier
                    .clickable {
                        editPlaylistVM.openModal()
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
        Box {
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
                        painter = painterResource(id = R.drawable.icon_adjust),
                        disabled = playlistsMode == PlaylistsMode.Adjust,
                        onClick = {
                            playlistsVM.toggleMode()
                        }
                    )
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Default,
                        painter = painterResource(id = R.drawable.icon_plus),
                        disabled = playlistsMode == PlaylistsMode.Adjust,
                        onClick = {
                            editPlaylistVM.openModal()
                        }
                    )
                }
                GridPlaylists()
            }
            if (playlistsMode == PlaylistsMode.Adjust) {
                FloatingActionButton(
                    containerColor = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .align(Alignment.BottomEnd)
                        .padding(32.dp),
                    onClick = {
                        playlistsVM.setMode(PlaylistsMode.Normal)
                    }
                ) {
                    Icon(
                        painter = painterResource(id = R.drawable.icon_yes),
                        tint = MaterialTheme.colorScheme.surface,
                        contentDescription = null,
                    )
                }
            }
        }
    }
}

@Composable
private fun GridPlaylists(
    playlistsVM: PlaylistsVM = hiltViewModel()
) {
    val playlists by playlistsVM.playlists.collectAsState()

    val lazyGridState = rememberLazyGridState()
    val reorderableLazyListState = rememberReorderableLazyGridState(lazyGridState = lazyGridState, scrollMoveMode = ScrollMoveMode.INSERT) { from, to ->
        playlistsVM.moveTo(from.index, to.index)
    }

    LazyVerticalGrid(
        modifier = Modifier.fillMaxSize(),
        columns = GridCells.FixedSize(172.dp),
        horizontalArrangement = Arrangement.Center,
        state = lazyGridState
    ) {
        items(playlists, key = { it.meta.id.value }) {
            ReorderableItem(reorderableLazyListState, key = it.meta.id.value) { isDragging ->
                PlaylistItem(playlist = it)
            }
        }
    }
}

@Composable
private fun ReorderableCollectionItemScope.PlaylistItem(
    playlist: PlaylistAbstract,
    playlistsVM: PlaylistsVM = hiltViewModel()
) {
    val mode by playlistsVM.mode.collectAsState()
    val navController = LocalNavController.current

    Box(Modifier
        .then(if (mode == PlaylistsMode.Adjust) {
            Modifier.draggableHandle()
        } else {
            Modifier.clickable(
                onClick = {
                    navController.navigate(RoutePlaylist(playlist.meta.id.value.toString()))
                },
            )
        })
    ) {
        Column(
            modifier = Modifier.padding(12.dp, 8.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.Start
        ) {
            Box(
                modifier = Modifier.clip(RoundedCornerShape(20.dp))
                    .background(MaterialTheme.colorScheme.onSurfaceVariant).size(136.dp)
            ) {
                val cover = playlist.meta.showCover
                if (cover == null) {
                    Image(
                        modifier = Modifier.fillMaxSize(),
                        painter = painterResource(id = R.drawable.cover_default_image),
                        contentDescription = null,
                        contentScale = ContentScale.FillWidth
                    )
                } else {
                    EaseImage(
                        modifier = Modifier.fillMaxSize(),
                        dataSourceKey = cover,
                        contentScale = ContentScale.FillWidth
                    )
                }
            }
            Row(
                modifier = Modifier.padding(top = 8.dp)
            ) {
                Text(
                    text = playlist.meta.title,
                    fontSize = 14.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            Text(
                text = buildAnnotatedString {
                    append("${playlist.musicCount} ${stringResource(id = R.string.music_count_unit)}")
                    append("  ·  ")
                    append(playlist.durationStr())
                },
                fontSize = 12.sp,
                fontWeight = FontWeight.Light,
                maxLines = 1,
            )
        }
        if (mode == PlaylistsMode.Adjust) {
            Box(
                modifier = Modifier
                    .align(Alignment.TopStart)
                    .size(24.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    modifier = Modifier.size(12.dp),
                    painter = painterResource(id = R.drawable.icon_drag),
                    tint = MaterialTheme.colorScheme.surface,
                    contentDescription = null,
                )
            }
        }
    }
}